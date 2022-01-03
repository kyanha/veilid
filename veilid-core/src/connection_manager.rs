use crate::connection_table::*;
use crate::intf::*;
use crate::network_manager::*;
use crate::xx::*;
use crate::*;
use futures_util::future::{select, Either};
use futures_util::stream::{FuturesUnordered, StreamExt};

const CONNECTION_PROCESSOR_CHANNEL_SIZE: usize = 128usize;

type ProtocolConnectHandler = fn(Option<SocketAddr>, DialInfo) -> Result<NetworkConnection, String>;

type ProtocolConnectorMap = BTreeMap<ProtocolType, ProtocolConnectHandler>;

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use async_std::net::*;
        use utils::async_peek_stream::*;

        pub trait ProtocolAcceptHandler: ProtocolAcceptHandlerClone + Send + Sync {
            fn on_accept(
                &self,
                stream: AsyncPeekStream,
                peer_addr: SocketAddr,
            ) -> SystemPinBoxFuture<Result<Option<NetworkConnection>, String>>;
        }

        pub trait ProtocolAcceptHandlerClone {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler>;
        }

        impl<T> ProtocolAcceptHandlerClone for T
        where
            T: 'static + ProtocolAcceptHandler + Clone,
        {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler> {
                Box::new(self.clone())
            }
        }
        impl Clone for Box<dyn ProtocolAcceptHandler> {
            fn clone(&self) -> Box<dyn ProtocolAcceptHandler> {
                self.clone_box()
            }
        }

        pub type NewProtocolAcceptHandler =
            dyn Fn(ConnectionManager, bool, SocketAddr) -> Box<dyn ProtocolAcceptHandler> + Send;
    }
}

pub struct ConnectionManagerInner {
    network_manager: NetworkManager,
    connection_table: ConnectionTable,
    connection_processor_jh: Option<JoinHandle<()>>,
    connection_add_channel_tx: Option<utils::channel::Sender<SystemPinBoxFuture<()>>>,
}

impl core::fmt::Debug for ConnectionManagerInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectionManagerInner")
            .field("connection_table", &self.connection_table)
            .finish()
    }
}

#[derive(Clone)]
pub struct ConnectionManager {
    inner: Arc<Mutex<ConnectionManagerInner>>,
}
impl core::fmt::Debug for ConnectionManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectionManager")
            .field("inner", &*self.inner.lock())
            .finish()
    }
}

impl ConnectionManager {
    fn new_inner(network_manager: NetworkManager) -> ConnectionManagerInner {
        ConnectionManagerInner {
            network_manager,
            connection_table: ConnectionTable::new(),
            connection_processor_jh: None,
            connection_add_channel_tx: None,
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
        }
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }

    pub fn config(&self) -> VeilidConfig {
        self.network_manager().config()
    }

    pub async fn startup(&self) {
        let cac = utils::channel::channel(CONNECTION_PROCESSOR_CHANNEL_SIZE); // xxx move to config
        self.inner.lock().connection_add_channel_tx = Some(cac.0);
        let rx = cac.1.clone();
        let this = self.clone();
        self.inner.lock().connection_processor_jh = Some(spawn(this.connection_processor(rx)));
    }

    pub async fn shutdown(&self) {
        *self.inner.lock() = Self::new_inner(self.network_manager());
    }

    // Returns a network connection if one already is established
    pub fn get_connection(&self, descriptor: &ConnectionDescriptor) -> Option<NetworkConnection> {
        self.inner
            .lock()
            .connection_table
            .get_connection(descriptor)
            .map(|e| e.conn)
    }

    // Called by low-level network when any connection-oriented protocol connection appears
    // either from incoming or outgoing connections. Registers connection in the connection table for later access
    // and spawns a message processing loop for the connection
    pub async fn on_new_connection(&self, conn: NetworkConnection) -> Result<(), String> {
        let tx = self
            .inner
            .lock()
            .connection_add_channel_tx
            .as_ref()
            .ok_or_else(fn_string!("connection channel isn't open yet"))?
            .clone();

        let receiver_loop_future = Self::process_connection(self.clone(), conn);
        tx.try_send(receiver_loop_future)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(error "failed to start receiver loop"))
    }

    // Connection receiver loop
    fn process_connection(
        this: ConnectionManager,
        conn: NetworkConnection,
    ) -> SystemPinBoxFuture<()> {
        let network_manager = this.network_manager();
        Box::pin(async move {
            // Add new connections to the table
            let entry = match this
                .inner
                .lock()
                .connection_table
                .add_connection(conn.clone())
            {
                Ok(e) => e,
                Err(err) => {
                    error!(target: "net", "{}", err);
                    return;
                }
            };

            //
            let exit_value: Result<Vec<u8>, ()> = Err(());
            let descriptor = conn.connection_descriptor();
            loop {
                let res = match select(
                    entry.stopper.clone().instance_clone(exit_value.clone()),
                    Box::pin(conn.clone().recv()),
                )
                .await
                {
                    Either::Left((_x, _b)) => break,
                    Either::Right((y, _a)) => y,
                };
                let message = match res {
                    Ok(v) => v,
                    Err(_) => break,
                };
                match network_manager
                    .on_recv_envelope(message.as_slice(), &descriptor)
                    .await
                {
                    Ok(_) => (),
                    Err(e) => {
                        error!("{}", e);
                        break;
                    }
                };
            }

            if let Err(err) = this
                .inner
                .lock()
                .connection_table
                .remove_connection(&descriptor)
            {
                error!("{}", err);
            }
        })
    }

    // Process connection oriented sockets in the background
    // This never terminates and must have its task cancelled once started
    // Task cancellation is performed by shutdown() by dropping the join handle
    async fn connection_processor(self, rx: utils::channel::Receiver<SystemPinBoxFuture<()>>) {
        let mut connection_futures: FuturesUnordered<SystemPinBoxFuture<()>> =
            FuturesUnordered::new();
        loop {
            // Either process an existing connection, or receive a new one to add to our list
            match select(connection_futures.next(), Box::pin(rx.recv())).await {
                Either::Left((x, _)) => {
                    // Processed some connection to completion, or there are none left
                    match x {
                        Some(()) => {
                            // Processed some connection to completion
                        }
                        None => {
                            // No connections to process, wait for one
                            match rx.recv().await {
                                Ok(v) => {
                                    connection_futures.push(v);
                                }
                                Err(e) => {
                                    log_net!(error "connection processor error: {:?}", e);
                                    // xxx: do something here?? should the network be restarted if this happens?
                                }
                            };
                        }
                    }
                }
                Either::Right((x, _)) => {
                    // Got a new connection future
                    match x {
                        Ok(v) => {
                            connection_futures.push(v);
                        }
                        Err(e) => {
                            log_net!(error "connection processor error: {:?}", e);
                            // xxx: do something here?? should the network be restarted if this happens?
                        }
                    };
                }
            }
        }
    }
}

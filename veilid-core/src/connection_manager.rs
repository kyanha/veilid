use crate::connection_table::*;
use crate::intf::*;
use crate::network_connection::*;
use crate::network_manager::*;
use crate::xx::*;
use crate::*;
use futures_util::stream::{FuturesUnordered, StreamExt};
use futures_util::{select, FutureExt};

const CONNECTION_PROCESSOR_CHANNEL_SIZE: usize = 128usize;

///////////////////////////////////////////////////////////
// Connection manager

struct ConnectionManagerInner {
    connection_table: ConnectionTable,
    connection_processor_jh: Option<JoinHandle<()>>,
    connection_add_channel_tx: Option<flume::Sender<SystemPinBoxFuture<()>>>,
}

impl core::fmt::Debug for ConnectionManagerInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectionManagerInner")
            .field("connection_table", &self.connection_table)
            .finish()
    }
}

struct ConnectionManagerArc {
    network_manager: NetworkManager,
    inner: AsyncMutex<ConnectionManagerInner>,
}
impl core::fmt::Debug for ConnectionManagerArc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectionManagerArc")
            .field("inner", &self.inner)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionManager {
    arc: Arc<ConnectionManagerArc>,
}

impl ConnectionManager {
    fn new_inner(config: VeilidConfig) -> ConnectionManagerInner {
        ConnectionManagerInner {
            connection_table: ConnectionTable::new(config),
            connection_processor_jh: None,
            connection_add_channel_tx: None,
        }
    }
    fn new_arc(network_manager: NetworkManager) -> ConnectionManagerArc {
        let config = network_manager.config();
        ConnectionManagerArc {
            network_manager,
            inner: AsyncMutex::new(Self::new_inner(config)),
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            arc: Arc::new(Self::new_arc(network_manager)),
        }
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.arc.network_manager.clone()
    }

    pub async fn startup(&self) {
        trace!("startup connection manager");
        let mut inner = self.arc.inner.lock().await;
        let cac = flume::bounded(CONNECTION_PROCESSOR_CHANNEL_SIZE);
        inner.connection_add_channel_tx = Some(cac.0);
        let rx = cac.1.clone();
        let this = self.clone();
        inner.connection_processor_jh = Some(spawn(this.connection_processor(rx)));
    }

    pub async fn shutdown(&self) {
        *self.arc.inner.lock().await = Self::new_inner(self.arc.network_manager.config());
    }

    // Returns a network connection if one already is established
    pub async fn get_connection(
        &self,
        descriptor: ConnectionDescriptor,
    ) -> Option<NetworkConnection> {
        let mut inner = self.arc.inner.lock().await;
        inner.connection_table.get_connection(descriptor)
    }

    // Internal routine to register new connection atomically
    fn on_new_connection_internal(
        &self,
        inner: &mut ConnectionManagerInner,
        conn: NetworkConnection,
    ) -> Result<(), String> {
        let tx = inner
            .connection_add_channel_tx
            .as_ref()
            .ok_or_else(fn_string!("connection channel isn't open yet"))?
            .clone();

        let receiver_loop_future = Self::process_connection(self.clone(), conn.clone());
        tx.try_send(receiver_loop_future)
            .map_err(map_to_string)
            .map_err(logthru_net!(error "failed to start receiver loop"))?;

        // If the receiver loop started successfully,
        // add the new connection to the table
        inner.connection_table.add_connection(conn)
    }

    // Called by low-level network when any connection-oriented protocol connection appears
    // either from incoming or outgoing connections. Registers connection in the connection table for later access
    // and spawns a message processing loop for the connection
    pub async fn on_new_connection(&self, conn: NetworkConnection) -> Result<(), String> {
        let mut inner = self.arc.inner.lock().await;
        self.on_new_connection_internal(&mut *inner, conn)
    }

    pub async fn get_or_create_connection(
        &self,
        local_addr: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<NetworkConnection, String> {
        let peer_address = dial_info.to_peer_address();
        let descriptor = match local_addr {
            Some(la) => {
                ConnectionDescriptor::new(peer_address, SocketAddress::from_socket_addr(la))
            }
            None => ConnectionDescriptor::new_no_local(peer_address),
        };

        // If any connection to this remote exists that has the same protocol, return it
        // Any connection will do, we don't have to match the local address
        let mut inner = self.arc.inner.lock().await;

        if let Some(conn) = inner
            .connection_table
            .get_last_connection_by_remote(descriptor.remote)
        {
            return Ok(conn);
        }

        // If not, attempt new connection
        let conn = NetworkConnection::connect(local_addr, dial_info).await?;

        self.on_new_connection_internal(&mut *inner, conn.clone())?;

        Ok(conn)
    }

    // Connection receiver loop
    fn process_connection(
        this: ConnectionManager,
        conn: NetworkConnection,
    ) -> SystemPinBoxFuture<()> {
        log_net!("Starting process_connection loop for {:?}", conn);
        let network_manager = this.network_manager();
        Box::pin(async move {
            //
            let descriptor = conn.connection_descriptor();
            let inactivity_timeout = this
                .network_manager()
                .config()
                .get()
                .network
                .connection_inactivity_timeout_ms;
            loop {
                // process inactivity timeout on receives only
                // if you want a keepalive, it has to be requested from the other side
                let message = select! {
                    res = conn.recv().fuse() => {
                        match res {
                            Ok(v) => v,
                            Err(e) => {
                                log_net!(error e);
                                break;
                            }
                        }
                    }
                    _ = intf::sleep(inactivity_timeout).fuse()=> {
                        // timeout
                        log_net!("connection timeout on {:?}", descriptor);
                        break;
                    }
                };
                if let Err(e) = network_manager
                    .on_recv_envelope(message.as_slice(), descriptor)
                    .await
                {
                    log_net!(error e);
                    break;
                }
            }

            if let Err(e) = this
                .arc
                .inner
                .lock()
                .await
                .connection_table
                .remove_connection(descriptor)
            {
                log_net!(error e);
            }
        })
    }

    // Process connection oriented sockets in the background
    // This never terminates and must have its task cancelled once started
    // Task cancellation is performed by shutdown() by dropping the join handle
    async fn connection_processor(self, rx: flume::Receiver<SystemPinBoxFuture<()>>) {
        let mut connection_futures: FuturesUnordered<SystemPinBoxFuture<()>> =
            FuturesUnordered::new();
        loop {
            // Either process an existing connection, or receive a new one to add to our list
            select! {
                x = connection_futures.next().fuse() => {
                    // Processed some connection to completion, or there are none left
                    match x {
                        Some(()) => {
                            // Processed some connection to completion
                        }
                        None => {
                            // No connections to process, wait for one
                            match rx.recv_async().await {
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
                x = rx.recv_async().fuse() => {
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

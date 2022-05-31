use super::*;
use crate::xx::*;
use connection_table::*;
use network_connection::*;

const CONNECTION_PROCESSOR_CHANNEL_SIZE: usize = 128usize;

///////////////////////////////////////////////////////////
// Connection manager

#[derive(Debug)]
struct ConnectionManagerInner {
    connection_table: ConnectionTable,
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
        //let mut inner = self.arc.inner.lock().await;
    }

    pub async fn shutdown(&self) {
        // xxx close all connections in the connection table

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
        log_net!("on_new_connection_internal: {:?}", conn);
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
        log_net!(
            "== get_or_create_connection local_addr={:?} dial_info={:?}",
            local_addr.green(),
            dial_info.green()
        );

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
            log_net!(
                "== Returning existing connection local_addr={:?} peer_address={:?}",
                local_addr.green(),
                peer_address.green()
            );

            return Ok(conn);
        }

        // Drop any other protocols connections that have the same local addr
        // otherwise this connection won't succeed due to binding
        if let Some(local_addr) = local_addr {
            if local_addr.port() != 0 {
                for pt in [ProtocolType::TCP, ProtocolType::WS, ProtocolType::WSS] {
                    let pa = PeerAddress::new(descriptor.remote.socket_address, pt);
                    for conn in inner.connection_table.get_connections_by_remote(pa) {
                        let desc = conn.connection_descriptor();
                        let mut kill = false;
                        if let Some(conn_local) = desc.local {
                            if (local_addr.ip().is_unspecified()
                                || (local_addr.ip() == conn_local.to_ip_addr()))
                                && conn_local.port() == local_addr.port()
                            {
                                kill = true;
                            }
                        }
                        if kill {
                            log_net!(debug
                                ">< Terminating connection local_addr={:?} peer_address={:?}",
                                local_addr.green(),
                                pa.green()
                            );
                            conn.close().await?;
                        }
                    }
                }
            }
        }

        // Attempt new connection
        let conn = NetworkConnection::connect(local_addr, dial_info).await?;

        self.on_new_connection_internal(&mut *inner, conn.clone())?;

        Ok(conn)
    }

    // Connection receiver loop
    fn process_connection(
        this: ConnectionManager,
        conn: NetworkConnection,
    ) -> SystemPinBoxFuture<()> {
        log_net!("Starting process_connection loop for {:?}", conn.green());
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
                                log_net!(debug e);
                                break;
                            }
                        }
                    }
                    _ = intf::sleep(inactivity_timeout).fuse()=> {
                        // timeout
                        log_net!("connection timeout on {:?}", descriptor.green());
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

            log_net!(
                "== Connection loop finished local_addr={:?} remote={:?}",
                descriptor.local.green(),
                descriptor.remote.green()
            );

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
}

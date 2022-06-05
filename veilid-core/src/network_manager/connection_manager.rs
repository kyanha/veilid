use super::*;
use crate::xx::*;
use connection_table::*;
use network_connection::*;

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
        // Drops connection table, which drops all connections in it
        *self.arc.inner.lock().await = Self::new_inner(self.arc.network_manager.config());
    }

    // Returns a network connection if one already is established
    pub async fn get_connection(
        &self,
        descriptor: ConnectionDescriptor,
    ) -> Option<ConnectionHandle> {
        let mut inner = self.arc.inner.lock().await;
        inner.connection_table.get_connection(descriptor)
    }

    // Internal routine to register new connection atomically.
    // Registers connection in the connection table for later access
    // and spawns a message processing loop for the connection
    fn on_new_protocol_network_connection(
        &self,
        inner: &mut ConnectionManagerInner,
        conn: ProtocolNetworkConnection,
    ) -> Result<ConnectionHandle, String> {
        log_net!("on_new_protocol_network_connection: {:?}", conn);

        // Wrap with NetworkConnection object to start the connection processing loop
        let conn = NetworkConnection::from_protocol(self.clone(), conn);
        let handle = conn.get_handle();
        // Add to the connection table
        inner.connection_table.add_connection(conn)?;
        Ok(handle)
    }

    // Called by low-level network when any connection-oriented protocol connection appears
    // either from incoming connections.
    pub(super) async fn on_accepted_protocol_network_connection(
        &self,
        conn: ProtocolNetworkConnection,
    ) -> Result<(), String> {
        let mut inner = self.arc.inner.lock().await;
        self.on_new_protocol_network_connection(&mut *inner, conn)
            .map(drop)
    }

    // Called when we want to create a new connection or get the current one that already exists
    // This will kill off any connections that are in conflict with the new connection to be made
    // in order to make room for the new connection in the system's connection table
    pub async fn get_or_create_connection(
        &self,
        local_addr: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<ConnectionHandle, String> {
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
                    for desc in inner
                        .connection_table
                        .get_connection_descriptors_by_remote(pa)
                    {
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
                            if let Err(e) = inner.connection_table.remove_connection(descriptor) {
                                log_net!(error e);
                            }
                        }
                    }
                }
            }
        }

        // Attempt new connection
        let conn = ProtocolNetworkConnection::connect(local_addr, dial_info).await?;

        self.on_new_protocol_network_connection(&mut *inner, conn)
    }

    // Callback from network connection receive loop when it exits
    // cleans up the entry in the connection table
    pub(super) async fn report_connection_finished(&self, descriptor: ConnectionDescriptor) {
        let mut inner = self.arc.inner.lock().await;
        if let Err(e) = inner.connection_table.remove_connection(descriptor) {
            log_net!(error e);
        }
    }
}

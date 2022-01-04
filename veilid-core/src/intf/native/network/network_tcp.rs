use super::*;
use crate::intf::*;
use crate::network_connection::*;
use utils::clone_stream::*;

use async_tls::TlsAcceptor;

/////////////////////////////////////////////////////////////////

#[derive(Clone)]
pub struct ListenerState {
    pub protocol_handlers: Vec<Box<dyn ProtocolAcceptHandler + 'static>>,
    pub tls_protocol_handlers: Vec<Box<dyn ProtocolAcceptHandler + 'static>>,
    pub tls_acceptor: Option<TlsAcceptor>,
}

impl ListenerState {
    pub fn new() -> Self {
        Self {
            protocol_handlers: Vec::new(),
            tls_protocol_handlers: Vec::new(),
            tls_acceptor: None,
        }
    }
}

/////////////////////////////////////////////////////////////////

impl Network {
    fn get_or_create_tls_acceptor(&self) -> Result<TlsAcceptor, String> {
        if let Some(ts) = self.inner.lock().tls_acceptor.as_ref() {
            return Ok(ts.clone());
        }

        let server_config = self
            .load_server_config()
            .map_err(|e| format!("Couldn't create TLS configuration: {}", e))?;
        let acceptor = TlsAcceptor::from(Arc::new(server_config));
        self.inner.lock().tls_acceptor = Some(acceptor.clone());
        Ok(acceptor)
    }

    async fn try_tls_handlers(
        &self,
        tls_acceptor: &TlsAcceptor,
        stream: AsyncPeekStream,
        addr: SocketAddr,
        protocol_handlers: &[Box<dyn ProtocolAcceptHandler>],
        tls_connection_initial_timeout: u64,
    ) -> Result<Option<NetworkConnection>, String> {
        let ts = tls_acceptor
            .accept(stream)
            .await
            .map_err(map_to_string)
            .map_err(logthru_net!(debug "TLS stream failed handshake"))?;
        let ps = AsyncPeekStream::new(CloneStream::new(ts));
        let mut first_packet = [0u8; PEEK_DETECT_LEN];

        // Try the handlers but first get a chunk of data for them to process
        // Don't waste more than N seconds getting it though, in case someone
        // is trying to DoS us with a bunch of connections or something
        // read a chunk of the stream
        io::timeout(
            Duration::from_micros(tls_connection_initial_timeout),
            ps.peek_exact(&mut first_packet),
        )
        .await
        .map_err(map_to_string)
        .map_err(logthru_net!())?;

        self.try_handlers(ps, addr, protocol_handlers).await
    }

    async fn try_handlers(
        &self,
        stream: AsyncPeekStream,
        addr: SocketAddr,
        protocol_handlers: &[Box<dyn ProtocolAcceptHandler>],
    ) -> Result<Option<NetworkConnection>, String> {
        for ah in protocol_handlers.iter() {
            if let Some(nc) = ah.on_accept(stream.clone(), addr).await? {
                return Ok(Some(nc));
            }
        }
        Ok(None)
    }

    async fn spawn_socket_listener(&self, addr: SocketAddr) -> Result<(), String> {
        // Get config
        let (connection_initial_timeout, tls_connection_initial_timeout) = {
            let c = self.config.get();
            (
                c.network.connection_initial_timeout,
                c.network.tls.connection_initial_timeout,
            )
        };

        // Create a reusable socket with no linger time, and no delay
        let socket = new_bound_shared_tcp_socket(addr)?;
        // Listen on the socket
        socket
            .listen(128)
            .map_err(|e| format!("Couldn't listen on TCP socket: {}", e))?;

        // Make an async tcplistener from the socket2 socket
        let std_listener: std::net::TcpListener = socket.into();
        let listener = TcpListener::from(std_listener);

        trace!("spawn_socket_listener: binding successful to {}", addr);

        // Create protocol handler records
        let listener_state = Arc::new(RwLock::new(ListenerState::new()));
        self.inner
            .lock()
            .listener_states
            .insert(addr, listener_state.clone());

        // Spawn the socket task
        let this = self.clone();
        let connection_manager = self.connection_manager();

        ////////////////////////////////////////////////////////////
        let jh = spawn(async move {
            // moves listener object in and get incoming iterator
            // when this task exists, the listener will close the socket
            listener
                .incoming()
                .for_each_concurrent(None, |tcp_stream| async {
                    let tcp_stream = tcp_stream.unwrap();
                    let listener_state = listener_state.clone();
                    let connection_manager = connection_manager.clone();

                    // Limit the number of connections from the same IP address
                    // and the number of total connections
                    let addr = match tcp_stream.peer_addr() {
                        Ok(addr) => addr,
                        Err(e) => {
                            error!("failed to get peer address: {}", e);
                            return;
                        }
                    };
                    // XXX limiting

                    trace!("TCP connection from: {}", addr);

                    // Create a stream we can peek on
                    let ps = AsyncPeekStream::new(tcp_stream);

                    /////////////////////////////////////////////////////////////
                    let mut first_packet = [0u8; PEEK_DETECT_LEN];

                    // read a chunk of the stream
                    if io::timeout(
                        Duration::from_micros(connection_initial_timeout),
                        ps.peek_exact(&mut first_packet),
                    )
                    .await
                    .is_err()
                    {
                        // If we fail to get a packet within the connection initial timeout
                        // then we punt this connection
                        return;
                    }

                    // Run accept handlers on accepted stream

                    // Check is this could be TLS
                    let ls = listener_state.read().clone();
                    let conn = if ls.tls_acceptor.is_some() && first_packet[0] == 0x16 {
                        this.try_tls_handlers(
                            ls.tls_acceptor.as_ref().unwrap(),
                            ps,
                            addr,
                            &ls.tls_protocol_handlers,
                            tls_connection_initial_timeout,
                        )
                        .await
                    } else {
                        this.try_handlers(ps, addr, &ls.protocol_handlers).await
                    };
                    let conn = match conn {
                        Ok(Some(c)) => c,
                        Ok(None) => {
                            // No protocol handlers matched? drop it.
                            return;
                        }
                        Err(_) => {
                            // Failed to negotiate connection? drop it.
                            return;
                        }
                    };

                    // Register the new connection in the connection manager
                    if let Err(e) = connection_manager.on_new_connection(conn).await {
                        error!("failed to register new connection: {}", e);
                    }
                })
                .await;
            trace!("exited incoming loop for {}", addr);
            // Remove our listener state from this address if we're stopping
            this.inner.lock().listener_states.remove(&addr);
            trace!("listener state removed for {}", addr);

            // If this happened our low-level listener socket probably died
            // so it's time to restart the network
            this.inner.lock().network_needs_restart = true;
        });
        ////////////////////////////////////////////////////////////

        // Add to join handles
        self.add_to_join_handles(jh);

        Ok(())
    }

    /////////////////////////////////////////////////////////////////

    // TCP listener that multiplexes ports so multiple protocols can exist on a single port
    pub(super) async fn start_tcp_listener(
        &self,
        address: String,
        is_tls: bool,
        new_protocol_accept_handler: Box<NewProtocolAcceptHandler>,
    ) -> Result<Vec<SocketAddress>, String> {
        let mut out = Vec::<SocketAddress>::new();
        // convert to socketaddrs
        let mut sockaddrs = address
            .to_socket_addrs()
            .await
            .map_err(|e| format!("Unable to resolve address: {}\n{}", address, e))?;
        for addr in &mut sockaddrs {
            let ldi_addrs = Self::translate_unspecified_address(&*(self.inner.lock()), &addr);

            // see if we've already bound to this already
            // if not, spawn a listener
            if !self.inner.lock().listener_states.contains_key(&addr) {
                self.clone().spawn_socket_listener(addr).await?;
            }

            let ls = if let Some(ls) = self.inner.lock().listener_states.get_mut(&addr) {
                ls.clone()
            } else {
                panic!("this shouldn't happen");
            };

            if is_tls {
                if ls.read().tls_acceptor.is_none() {
                    ls.write().tls_acceptor = Some(self.clone().get_or_create_tls_acceptor()?);
                }
                ls.write()
                    .tls_protocol_handlers
                    .push(new_protocol_accept_handler(
                        self.network_manager().config(),
                        true,
                        addr,
                    ));
            } else {
                ls.write()
                    .protocol_handlers
                    .push(new_protocol_accept_handler(
                        self.network_manager().config(),
                        false,
                        addr,
                    ));
            }

            // Return local dial infos we listen on
            for ldi_addr in ldi_addrs {
                out.push(SocketAddress::from_socket_addr(ldi_addr));
            }
        }

        Ok(out)
    }
}

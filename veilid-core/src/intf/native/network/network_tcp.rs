use super::*;
use utils::clone_stream::*;

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
        protocol_handlers: &[Box<dyn TcpProtocolHandler>],
        tls_connection_initial_timeout: u64,
    ) {
        match tls_acceptor.accept(stream).await {
            Ok(ts) => {
                let ps = AsyncPeekStream::new(CloneStream::new(ts));
                let mut first_packet = [0u8; PEEK_DETECT_LEN];

                // Try the handlers but first get a chunk of data for them to process
                // Don't waste more than N seconds getting it though, in case someone
                // is trying to DoS us with a bunch of connections or something
                // read a chunk of the stream
                match io::timeout(
                    Duration::from_micros(tls_connection_initial_timeout),
                    ps.peek_exact(&mut first_packet),
                )
                .await
                {
                    Ok(()) => (),
                    Err(_) => return,
                }
                self.clone().try_handlers(ps, addr, protocol_handlers).await;
            }
            Err(e) => {
                debug!("TLS stream failed handshake: {}", e);
            }
        }
    }

    async fn try_handlers(
        &self,
        stream: AsyncPeekStream,
        addr: SocketAddr,
        protocol_handlers: &[Box<dyn TcpProtocolHandler>],
    ) {
        for ah in protocol_handlers.iter() {
            if ah.on_accept(stream.clone(), addr).await == Ok(true) {
                return;
            }
        }
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
        let socket = new_shared_tcp_socket(addr)?;
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

        ////////////////////////////////////////////////////////////
        let jh = spawn(async move {
            // moves listener object in and get incoming iterator
            // when this task exists, the listener will close the socket
            listener
                .incoming()
                .for_each_concurrent(None, |tcp_stream| async {
                    let tcp_stream = tcp_stream.unwrap();
                    let listener_state = listener_state.clone();
                    // match tcp_stream.set_nodelay(true) {
                    //     Ok(_) => (),
                    //     _ => continue,
                    // };

                    // Limit the number of connections from the same IP address
                    // and the number of total connections
                    let addr = match tcp_stream.peer_addr() {
                        Ok(addr) => addr,
                        Err(err) => {
                            error!("failed to get peer address: {}", err);
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
                    trace!("reading chunk");
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
                    trace!("packet ready");
                    // Check is this could be TLS
                    let ls = listener_state.read().clone();
                    if ls.tls_acceptor.is_some() && first_packet[0] == 0x16 {
                        trace!("trying TLS");
                        this.clone()
                            .try_tls_handlers(
                                ls.tls_acceptor.as_ref().unwrap(),
                                ps,
                                addr,
                                &ls.tls_protocol_handlers,
                                tls_connection_initial_timeout,
                            )
                            .await;
                    } else {
                        trace!("not TLS");
                        this.clone()
                            .try_handlers(ps, addr, &ls.protocol_handlers)
                            .await;
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
        new_tcp_protocol_handler: Box<NewTcpProtocolHandler>,
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
                    .push(new_tcp_protocol_handler(
                        self.inner.lock().network_manager.clone(),
                        true,
                        addr,
                    ));
            } else {
                ls.write().protocol_handlers.push(new_tcp_protocol_handler(
                    self.inner.lock().network_manager.clone(),
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

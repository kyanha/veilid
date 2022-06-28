use super::*;
use sockets::*;
use stop_token::future::FutureExt;

impl Network {
    pub(super) async fn create_udp_listener_tasks(&self) -> Result<(), String> {
        // Spawn socket tasks
        let mut task_count = {
            let c = self.config.get();
            c.network.protocol.udp.socket_pool_size
        };
        if task_count == 0 {
            task_count = intf::get_concurrency() / 2;
            if task_count == 0 {
                task_count = 1;
            }
        }
        trace!("task_count: {}", task_count);
        for _ in 0..task_count {
            trace!("Spawning UDP listener task");

            ////////////////////////////////////////////////////////////
            // Run thread task to process stream of messages
            let this = self.clone();

            let jh = spawn_with_local_set(async move {
                trace!("UDP listener task spawned");

                // Collect all our protocol handlers into a vector
                let mut protocol_handlers: Vec<RawUdpProtocolHandler> = this
                    .inner
                    .lock()
                    .inbound_udp_protocol_handlers
                    .values()
                    .cloned()
                    .collect();
                if let Some(ph) = this.inner.lock().outbound_udpv4_protocol_handler.clone() {
                    protocol_handlers.push(ph);
                }
                if let Some(ph) = this.inner.lock().outbound_udpv6_protocol_handler.clone() {
                    protocol_handlers.push(ph);
                }

                // Spawn a local async task for each socket
                let mut protocol_handlers_unordered = FuturesUnordered::new();
                let network_manager = this.network_manager();
                let stop_token = this.inner.lock().stop_source.as_ref().unwrap().token();

                for ph in protocol_handlers {
                    let network_manager = network_manager.clone();
                    let stop_token = stop_token.clone();
                    let jh = intf::spawn_local(async move {
                        let mut data = vec![0u8; 65536];

                        loop {
                            match ph
                                .recv_message(&mut data)
                                .timeout_at(stop_token.clone())
                                .await
                            {
                                Ok(Ok((size, descriptor))) => {
                                    // XXX: Limit the number of packets from the same IP address?
                                    log_net!("UDP packet: {:?}", descriptor);

                                    // Network accounting
                                    network_manager.stats_packet_rcvd(
                                        descriptor.remote_address().to_ip_addr(),
                                        size as u64,
                                    );

                                    // Pass it up for processing
                                    if let Err(e) = network_manager
                                        .on_recv_envelope(&data[..size], descriptor)
                                        .await
                                    {
                                        log_net!(error "failed to process received udp envelope: {}", e);
                                    }
                                }
                                Ok(Err(_)) => {
                                    return false;
                                }
                                Err(_) => {
                                    return true;
                                }
                            }
                        }
                    });

                    protocol_handlers_unordered.push(jh);
                }
                // Now we wait for join handles to exit,
                // if any error out it indicates an error needing
                // us to completely restart the network
                loop {
                    match protocol_handlers_unordered.next().await {
                        Some(v) => {
                            // true = stopped, false = errored
                            if !v {
                                // If any protocol handler fails, our socket died and we need to restart the network
                                this.inner.lock().network_needs_restart = true;
                            }
                        }
                        None => {
                            // All protocol handlers exited
                            break;
                        }
                    }
                }

                trace!("UDP listener task stopped");
            });
            ////////////////////////////////////////////////////////////

            // Add to join handle
            self.add_to_join_handles(jh);
        }

        Ok(())
    }

    pub(super) async fn create_udp_outbound_sockets(&self) -> Result<(), String> {
        let mut inner = self.inner.lock();
        let mut port = inner.udp_port;
        // v4
        let socket_addr_v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
        if let Ok(socket) = new_bound_shared_udp_socket(socket_addr_v4) {
            // Pull the port if we randomly bound, so v6 can be on the same port
            port = socket
                .local_addr()
                .map_err(map_to_string)?
                .as_socket_ipv4()
                .ok_or_else(|| "expected ipv4 address type".to_owned())?
                .port();

            // Make an async UdpSocket from the socket2 socket
            let std_udp_socket: std::net::UdpSocket = socket.into();
            cfg_if! {
                if #[cfg(feature="rt-async-std")] {
                    let udp_socket = UdpSocket::from(std_udp_socket);
                } else if #[cfg(feature="rt-tokio")] {
                    let udp_socket = UdpSocket::from_std(std_udp_socket).map_err(map_to_string)?;
                }
            }
            let socket_arc = Arc::new(udp_socket);

            // Create protocol handler
            let udpv4_handler = RawUdpProtocolHandler::new(socket_arc);

            inner.outbound_udpv4_protocol_handler = Some(udpv4_handler);
        }
        //v6
        let socket_addr_v6 =
            SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), port);
        if let Ok(socket) = new_bound_shared_udp_socket(socket_addr_v6) {
            // Make an async UdpSocket from the socket2 socket
            let std_udp_socket: std::net::UdpSocket = socket.into();
            cfg_if! {
                if #[cfg(feature="rt-async-std")] {
                    let udp_socket = UdpSocket::from(std_udp_socket);
                } else if #[cfg(feature="rt-tokio")] {
                    let udp_socket = UdpSocket::from_std(std_udp_socket).map_err(map_to_string)?;
                }
            }
            let socket_arc = Arc::new(udp_socket);

            // Create protocol handler
            let udpv6_handler = RawUdpProtocolHandler::new(socket_arc);

            inner.outbound_udpv6_protocol_handler = Some(udpv6_handler);
        }

        Ok(())
    }

    async fn create_udp_inbound_socket(&self, addr: SocketAddr) -> Result<(), String> {
        log_net!("create_udp_inbound_socket on {:?}", &addr);

        // Create a reusable socket
        let socket = new_bound_shared_udp_socket(addr)?;

        // Make an async UdpSocket from the socket2 socket
        let std_udp_socket: std::net::UdpSocket = socket.into();
        cfg_if! {
            if #[cfg(feature="rt-async-std")] {
                let udp_socket = UdpSocket::from(std_udp_socket);
            } else if #[cfg(feature="rt-tokio")] {
                let udp_socket = UdpSocket::from_std(std_udp_socket).map_err(map_to_string)?;
            }
        }
        let socket_arc = Arc::new(udp_socket);

        // Create protocol handler
        let protocol_handler = RawUdpProtocolHandler::new(socket_arc);

        // Create message_handler records
        self.inner
            .lock()
            .inbound_udp_protocol_handlers
            .insert(addr, protocol_handler);

        Ok(())
    }

    pub(super) async fn create_udp_inbound_sockets(
        &self,
        ip_addrs: Vec<IpAddr>,
        port: u16,
    ) -> Result<Vec<DialInfo>, String> {
        let mut out = Vec::<DialInfo>::new();

        for ip_addr in ip_addrs {
            let addr = SocketAddr::new(ip_addr, port);

            // see if we've already bound to this already
            // if not, spawn a listener
            if !self
                .inner
                .lock()
                .inbound_udp_protocol_handlers
                .contains_key(&addr)
            {
                let idi_addrs = Self::translate_unspecified_address(&*self.inner.lock(), &addr);

                self.clone().create_udp_inbound_socket(addr).await?;

                // Return interface dial infos we listen on
                for idi_addr in idi_addrs {
                    out.push(DialInfo::udp_from_socketaddr(idi_addr));
                }
            }
        }
        Ok(out)
    }

    /////////////////////////////////////////////////////////////////

    pub(super) fn find_best_udp_protocol_handler(
        &self,
        peer_socket_addr: &SocketAddr,
        local_socket_addr: &Option<SocketAddr>,
    ) -> Option<RawUdpProtocolHandler> {
        // if our last communication with this peer came from a particular inbound udp protocol handler, use it
        if let Some(sa) = local_socket_addr {
            if let Some(ph) = self.inner.lock().inbound_udp_protocol_handlers.get(sa) {
                return Some(ph.clone());
            }
        }

        // otherwise find the outbound udp protocol handler that matches the ip protocol version of the peer addr
        let inner = self.inner.lock();
        match peer_socket_addr {
            SocketAddr::V4(_) => inner.outbound_udpv4_protocol_handler.clone(),
            SocketAddr::V6(_) => inner.outbound_udpv6_protocol_handler.clone(),
        }
    }
}

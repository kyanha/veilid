use super::*;

impl Network {
    /////////////////////////////////////////////////////
    // Support for binding first on ports to ensure nobody binds ahead of us
    // or two copies of the app don't accidentally collide. This is tricky
    // because we use 'reuseaddr/port' and we can accidentally bind in front of ourselves :P

    fn bind_first_udp_port(&self, udp_port: u16) -> bool {
        let mut inner = self.inner.lock();
        if inner.bound_first_udp.contains_key(&udp_port) {
            return true;
        }
        // If the address is specified, only use the specified port and fail otherwise
        let mut bound_first_socket_v4 = None;
        let mut bound_first_socket_v6 = None;
        if let Ok(bfs4) =
            new_bound_first_udp_socket(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), udp_port))
        {
            if let Ok(bfs6) = new_bound_first_udp_socket(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                udp_port,
            )) {
                bound_first_socket_v4 = Some(bfs4);
                bound_first_socket_v6 = Some(bfs6);
            }
        }
        if let (Some(bfs4), Some(bfs6)) = (bound_first_socket_v4, bound_first_socket_v6) {
            inner.bound_first_udp.insert(udp_port, (bfs4, bfs6));
            true
        } else {
            false
        }
    }

    fn bind_first_tcp_port(&self, tcp_port: u16) -> bool {
        let mut inner = self.inner.lock();
        if inner.bound_first_tcp.contains_key(&tcp_port) {
            return true;
        }
        // If the address is specified, only use the specified port and fail otherwise
        let mut bound_first_socket_v4 = None;
        let mut bound_first_socket_v6 = None;
        if let Ok(bfs4) =
            new_bound_first_tcp_socket(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), tcp_port))
        {
            if let Ok(bfs6) = new_bound_first_tcp_socket(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                tcp_port,
            )) {
                bound_first_socket_v4 = Some(bfs4);
                bound_first_socket_v6 = Some(bfs6);
            }
        }
        if let (Some(bfs4), Some(bfs6)) = (bound_first_socket_v4, bound_first_socket_v6) {
            inner.bound_first_tcp.insert(tcp_port, (bfs4, bfs6));
            true
        } else {
            false
        }
    }

    pub(super) fn free_bound_first_ports(&self) {
        let mut inner = self.inner.lock();
        inner.bound_first_udp.clear();
        inner.bound_first_tcp.clear();
    }

    /////////////////////////////////////////////////////

    fn find_available_udp_port(&self) -> Result<u16, String> {
        // If the address is empty, iterate ports until we find one we can use.
        let mut udp_port = 5150u16;
        loop {
            if self.bind_first_udp_port(udp_port) {
                break;
            }
            if udp_port == 65535 {
                return Err("Could not find free udp port to listen on".to_owned());
            }
            udp_port += 1;
        }
        Ok(udp_port)
    }

    fn find_available_tcp_port(&self) -> Result<u16, String> {
        // If the address is empty, iterate ports until we find one we can use.
        let mut tcp_port = 5150u16;
        loop {
            if self.bind_first_tcp_port(tcp_port) {
                break;
            }
            if tcp_port == 65535 {
                return Err("Could not find free tcp port to listen on".to_owned());
            }
            tcp_port += 1;
        }
        Ok(tcp_port)
    }

    async fn allocate_udp_port(
        &self,
        listen_address: String,
    ) -> Result<(u16, Vec<IpAddr>), String> {
        if listen_address.is_empty() {
            // If listen address is empty, find us a port iteratively
            let port = self.find_available_udp_port()?;
            let ip_addrs = vec![
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            ];
            Ok((port, ip_addrs))
        } else {
            // If the address is specified, only use the specified port and fail otherwise
            let sockaddrs: Vec<SocketAddr> = listen_address
                .to_socket_addrs()
                .await
                .map_err(|e| format!("Unable to resolve address: {}\n{}", listen_address, e))?
                .collect();

            if sockaddrs.is_empty() {
                Err(format!("No valid listen address: {}", listen_address))
            } else {
                let port = sockaddrs[0].port();
                if self.bind_first_udp_port(port) {
                    Ok((port, sockaddrs.iter().map(|s| s.ip()).collect()))
                } else {
                    Err("Could not find free udp port to listen on".to_owned())
                }
            }
        }
    }

    async fn allocate_tcp_port(
        &self,
        listen_address: String,
    ) -> Result<(u16, Vec<IpAddr>), String> {
        if listen_address.is_empty() {
            // If listen address is empty, find us a port iteratively
            let port = self.find_available_tcp_port()?;
            let ip_addrs = vec![
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            ];
            Ok((port, ip_addrs))
        } else {
            // If the address is specified, only use the specified port and fail otherwise
            let sockaddrs: Vec<SocketAddr> = listen_address
                .to_socket_addrs()
                .await
                .map_err(|e| format!("Unable to resolve address: {}\n{}", listen_address, e))?
                .collect();

            if sockaddrs.is_empty() {
                Err(format!("No valid listen address: {}", listen_address))
            } else {
                let port = sockaddrs[0].port();
                if self.bind_first_tcp_port(port) {
                    Ok((port, sockaddrs.iter().map(|s| s.ip()).collect()))
                } else {
                    Err("Could not find free tcp port to listen on".to_owned())
                }
            }
        }
    }

    /////////////////////////////////////////////////////

    pub(super) async fn start_udp_listeners(&self) -> Result<(), String> {
        let routing_table = self.routing_table();
        let (listen_address, public_address) = {
            let c = self.config.get();
            (
                c.network.protocol.udp.listen_address.clone(),
                c.network.protocol.udp.public_address.clone(),
            )
        };

        // Pick out UDP port we're going to use everywhere
        // Keep sockets around until the end of this function
        // to keep anyone else from binding in front of us
        let (udp_port, ip_addrs) = self.allocate_udp_port(listen_address.clone()).await?;

        // Save the bound udp port for use later on
        self.inner.lock().udp_port = udp_port;

        // First, create outbound sockets
        // (unlike tcp where we create sockets for every connection)
        // and we'll add protocol handlers for them too
        self.create_udp_outbound_sockets().await?;

        // Now create udp inbound sockets for whatever interfaces we're listening on
        info!(
            "UDP: starting listeners on port {} at {:?}",
            udp_port, ip_addrs
        );
        let dial_infos = self.create_udp_inbound_sockets(ip_addrs, udp_port).await?;
        let mut static_public = false;
        for di in &dial_infos {
            // Register local dial info only here if we specify a public address
            if public_address.is_none() && di.is_global() {
                // Register global dial info if no public address is specified
                routing_table.register_dial_info(
                    di.clone(),
                    DialInfoOrigin::Static,
                    Some(NetworkClass::Server),
                );
                static_public = true;
            } else if di.is_local() {
                // Register local dial info
                routing_table.register_dial_info(di.clone(), DialInfoOrigin::Static, None);
            }
        }

        // Add static public dialinfo if it's configured
        if let Some(public_address) = public_address.as_ref() {
            // Resolve statically configured public dialinfo
            let mut public_sockaddrs = public_address
                .to_socket_addrs()
                .await
                .map_err(|e| format!("Unable to resolve address: {}\n{}", public_address, e))?;

            // Add all resolved addresses as public dialinfo
            for pdi_addr in &mut public_sockaddrs {
                routing_table.register_dial_info(
                    DialInfo::udp_from_socketaddr(pdi_addr),
                    DialInfoOrigin::Static,
                    Some(NetworkClass::Server),
                );
                static_public = true;
            }
        }

        self.inner.lock().udp_static_public_dialinfo = static_public;

        // Now create tasks for udp listeners
        self.create_udp_listener_tasks().await
    }

    pub(super) async fn start_ws_listeners(&self) -> Result<(), String> {
        let routing_table = self.routing_table();
        let (listen_address, url, path) = {
            let c = self.config.get();
            (
                c.network.protocol.ws.listen_address.clone(),
                c.network.protocol.ws.url.clone(),
                c.network.protocol.ws.path.clone(),
            )
        };

        // Pick out TCP port we're going to use everywhere
        // Keep sockets around until the end of this function
        // to keep anyone else from binding in front of us
        let (ws_port, ip_addrs) = self.allocate_tcp_port(listen_address.clone()).await?;

        // Save the bound ws port for use later on
        self.inner.lock().ws_port = ws_port;

        trace!(
            "WS: starting listener on port {} at {:?}",
            ws_port,
            ip_addrs
        );
        let socket_addresses = self
            .start_tcp_listener(
                ip_addrs,
                ws_port,
                false,
                Box::new(|c, t, a| Box::new(WebsocketProtocolHandler::new(c, t, a))),
            )
            .await?;
        trace!("WS: listener started");

        let mut static_public = false;
        for socket_address in socket_addresses {
            if url.is_none() && socket_address.address().is_global() {
                // Build global dial info request url
                let global_url = format!("ws://{}/{}", socket_address, path);

                // Create global dial info
                let di = DialInfo::try_ws(socket_address, global_url)
                    .map_err(map_to_string)
                    .map_err(logthru_net!(error))?;
                routing_table.register_dial_info(
                    di,
                    DialInfoOrigin::Static,
                    Some(NetworkClass::Server),
                );
                static_public = true;
            } else if socket_address.address().is_local() {
                // Build local dial info request url
                let local_url = format!("ws://{}/{}", socket_address, path);

                // Create local dial info
                let di = DialInfo::try_ws(socket_address, local_url)
                    .map_err(map_to_string)
                    .map_err(logthru_net!(error))?;
                routing_table.register_dial_info(di, DialInfoOrigin::Static, None);
            }
        }

        // Add static public dialinfo if it's configured
        if let Some(url) = url.as_ref() {
            let mut split_url = SplitUrl::from_str(url)?;
            if split_url.scheme.to_ascii_lowercase() != "ws" {
                return Err("WS URL must use 'ws://' scheme".to_owned());
            }
            split_url.scheme = "ws".to_owned();

            // Resolve static public hostnames
            let global_socket_addrs = split_url
                .host
                .to_socket_addrs()
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;

            for gsa in global_socket_addrs {
                routing_table.register_dial_info(
                    DialInfo::try_ws(SocketAddress::from_socket_addr(gsa), url.clone())
                        .map_err(map_to_string)
                        .map_err(logthru_net!(error))?,
                    DialInfoOrigin::Static,
                    Some(NetworkClass::Server),
                );
            }
            static_public = true;
        }
        self.inner.lock().ws_static_public_dialinfo = static_public;

        Ok(())
    }

    pub(super) async fn start_wss_listeners(&self) -> Result<(), String> {
        let routing_table = self.routing_table();
        let (listen_address, url) = {
            let c = self.config.get();
            (
                c.network.protocol.wss.listen_address.clone(),
                c.network.protocol.wss.url.clone(),
            )
        };

        // Pick out TCP port we're going to use everywhere
        // Keep sockets around until the end of this function
        // to keep anyone else from binding in front of us
        let (wss_port, ip_addrs) = self.allocate_tcp_port(listen_address.clone()).await?;

        // Save the bound wss port for use later on
        self.inner.lock().wss_port = wss_port;

        trace!(
            "WSS: starting listener on port {} at {:?}",
            wss_port,
            ip_addrs
        );
        let _socket_addresses = self
            .start_tcp_listener(
                ip_addrs,
                wss_port,
                true,
                Box::new(|c, t, a| Box::new(WebsocketProtocolHandler::new(c, t, a))),
            )
            .await?;
        trace!("WSS: listener started");

        // NOTE: No local dial info for WSS, as there is no way to connect to a local dialinfo via TLS
        // If the hostname is specified, it is the public dialinfo via the URL. If no hostname
        // is specified, then TLS won't validate, so no local dialinfo is possible.
        // This is not the case with unencrypted websockets, which can be specified solely by an IP address

        // Add static public dialinfo if it's configured
        if let Some(url) = url.as_ref() {
            // Add static public dialinfo if it's configured
            let mut split_url = SplitUrl::from_str(url)?;
            if split_url.scheme.to_ascii_lowercase() != "wss" {
                return Err("WSS URL must use 'wss://' scheme".to_owned());
            }
            split_url.scheme = "wss".to_owned();

            // Resolve static public hostnames
            let global_socket_addrs = split_url
                .host
                .to_socket_addrs()
                .await
                .map_err(map_to_string)
                .map_err(logthru_net!(error))?;

            for gsa in global_socket_addrs {
                routing_table.register_dial_info(
                    DialInfo::try_wss(SocketAddress::from_socket_addr(gsa), url.clone())
                        .map_err(map_to_string)
                        .map_err(logthru_net!(error))?,
                    DialInfoOrigin::Static,
                    Some(NetworkClass::Server),
                );
            }
        } else {
            return Err("WSS URL must be specified due to TLS requirements".to_owned());
        }

        Ok(())
    }

    pub(super) async fn start_tcp_listeners(&self) -> Result<(), String> {
        let routing_table = self.routing_table();
        let (listen_address, public_address) = {
            let c = self.config.get();
            (
                c.network.protocol.tcp.listen_address.clone(),
                c.network.protocol.tcp.public_address.clone(),
            )
        };

        // Pick out TCP port we're going to use everywhere
        // Keep sockets around until the end of this function
        // to keep anyone else from binding in front of us
        let (tcp_port, ip_addrs) = self.allocate_tcp_port(listen_address.clone()).await?;

        // Save the bound tcp port for use later on
        self.inner.lock().tcp_port = tcp_port;

        trace!(
            "TCP: starting listener on port {} at {:?}",
            tcp_port,
            ip_addrs
        );
        let socket_addresses = self
            .start_tcp_listener(
                ip_addrs,
                tcp_port,
                false,
                Box::new(|_, _, a| Box::new(RawTcpProtocolHandler::new(a))),
            )
            .await?;
        trace!("TCP: listener started");

        let mut static_public = false;
        for socket_address in socket_addresses {
            let di = DialInfo::tcp(socket_address);

            // Register local dial info only here if we specify a public address
            if public_address.is_none() && di.is_global() {
                // Register global dial info if no public address is specified
                routing_table.register_dial_info(
                    di.clone(),
                    DialInfoOrigin::Static,
                    Some(NetworkClass::Server),
                );
                static_public = true;
            } else if di.is_local() {
                // Register local dial info
                routing_table.register_dial_info(di.clone(), DialInfoOrigin::Static, None);
            }
        }

        // Add static public dialinfo if it's configured
        if let Some(public_address) = public_address.as_ref() {
            // Resolve statically configured public dialinfo
            let mut public_sockaddrs = public_address
                .to_socket_addrs()
                .await
                .map_err(|e| format!("Unable to resolve address: {}\n{}", public_address, e))?;

            // Add all resolved addresses as public dialinfo
            for pdi_addr in &mut public_sockaddrs {
                routing_table.register_dial_info(
                    DialInfo::tcp_from_socketaddr(pdi_addr),
                    DialInfoOrigin::Static,
                    None,
                );
                static_public = true;
            }
        }

        self.inner.lock().tcp_static_public_dialinfo = static_public;

        Ok(())
    }
}

use super::*;

impl Network {
    pub(super) async fn start_udp_listeners(&self) -> Result<(), String> {
        // First, create outbound sockets and we'll listen on them too
        self.create_udp_outbound_sockets().await?;

        // Now create udp inbound sockets for whatever interfaces we're listening on
        let routing_table = self.routing_table();
        let (listen_address, public_address) = {
            let c = self.config.get();
            (
                c.network.protocol.udp.listen_address.clone(),
                c.network.protocol.udp.public_address.clone(),
            )
        };
        info!("UDP: starting listener at {:?}", listen_address);
        let dial_infos = self
            .create_udp_inbound_sockets(listen_address.clone())
            .await?;
        let mut static_public = false;
        for di in &dial_infos {
            // Pick out UDP port for outbound connections (they will all be the same)
            self.inner.lock().udp_port = di.port();

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
        trace!("WS: starting listener at {:?}", listen_address);
        let socket_addresses = self
            .start_tcp_listener(
                listen_address.clone(),
                false,
                Box::new(|n, t, a| Box::new(WebsocketProtocolHandler::new(n, t, a))),
            )
            .await?;
        trace!("WS: listener started");

        let mut static_public = false;
        for socket_address in socket_addresses {
            // Pick out WS port for outbound connections (they will all be the same)
            self.inner.lock().ws_port = socket_address.port();

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
        trace!("WSS: starting listener at {}", listen_address);
        let socket_addresses = self
            .start_tcp_listener(
                listen_address.clone(),
                true,
                Box::new(|n, t, a| Box::new(WebsocketProtocolHandler::new(n, t, a))),
            )
            .await?;
        trace!("WSS: listener started");

        // NOTE: No local dial info for WSS, as there is no way to connect to a local dialinfo via TLS
        // If the hostname is specified, it is the public dialinfo via the URL. If no hostname
        // is specified, then TLS won't validate, so no local dialinfo is possible.
        // This is not the case with unencrypted websockets, which can be specified solely by an IP address
        //
        if let Some(socket_address) = socket_addresses.first() {
            // Pick out WSS port for outbound connections (they will all be the same)
            self.inner.lock().wss_port = socket_address.port();
        }

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
        trace!("TCP: starting listener at {}", &listen_address);
        let socket_addresses = self
            .start_tcp_listener(
                listen_address.clone(),
                false,
                Box::new(|n, _, a| Box::new(RawTcpProtocolHandler::new(n, a))),
            )
            .await?;
        trace!("TCP: listener started");

        let mut static_public = false;
        for socket_address in socket_addresses {
            // Pick out TCP port for outbound connections (they will all be the same)
            self.inner.lock().tcp_port = socket_address.port();

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

mod listener_state;
mod protocol;
mod public_dialinfo_discovery;

use crate::intf::*;
use crate::network_manager::*;
use crate::routing_table::*;
use crate::*;
use listener_state::*;
use protocol::tcp::RawTcpProtocolHandler;
use protocol::udp::RawUdpProtocolHandler;
use protocol::ws::WebsocketProtocolHandler;
pub use protocol::*;
use utils::async_peek_stream::*;
use utils::clone_stream::*;
use utils::network_interfaces::*;

use async_std::io;
use async_std::net::*;
use async_tls::TlsAcceptor;
use futures_util::StreamExt;
// xxx: rustls ^0.20
//use rustls::{server::NoClientAuth, Certificate, PrivateKey, ServerConfig};
use rustls::{Certificate, NoClientAuth, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

/////////////////////////////////////////////////////////////////

pub const PEEK_DETECT_LEN: usize = 64;

/////////////////////////////////////////////////////////////////

struct NetworkInner {
    routing_table: RoutingTable,
    network_manager: NetworkManager,
    network_started: bool,
    network_needs_restart: bool,
    protocol_config: Option<ProtocolConfig>,
    udp_static_public_dialinfo: bool,
    tcp_static_public_dialinfo: bool,
    ws_static_public_dialinfo: bool,
    network_class: Option<NetworkClass>,
    join_handles: Vec<JoinHandle<()>>,
    listener_states: BTreeMap<SocketAddr, Arc<RwLock<ListenerState>>>,
    udp_protocol_handlers: BTreeMap<SocketAddr, RawUdpProtocolHandler>,
    tls_acceptor: Option<TlsAcceptor>,
    udp_port: u16,
    tcp_port: u16,
    ws_port: u16,
    wss_port: u16,
    outbound_udpv4_protocol_handler: Option<RawUdpProtocolHandler>,
    outbound_udpv6_protocol_handler: Option<RawUdpProtocolHandler>,
    interfaces: NetworkInterfaces,
}

struct NetworkUnlockedInner {
    // Background processes
    update_udpv4_dialinfo_task: TickTask,
    update_tcpv4_dialinfo_task: TickTask,
}

#[derive(Clone)]
pub struct Network {
    config: VeilidConfig,
    inner: Arc<Mutex<NetworkInner>>,
    unlocked_inner: Arc<NetworkUnlockedInner>,
}

impl Network {
    fn new_inner(network_manager: NetworkManager) -> NetworkInner {
        NetworkInner {
            routing_table: network_manager.routing_table(),
            network_manager,
            network_started: false,
            network_needs_restart: false,
            protocol_config: None,
            udp_static_public_dialinfo: false,
            tcp_static_public_dialinfo: false,
            ws_static_public_dialinfo: false,
            network_class: None,
            join_handles: Vec::new(),
            listener_states: BTreeMap::new(),
            udp_protocol_handlers: BTreeMap::new(),
            tls_acceptor: None,
            udp_port: 0u16,
            tcp_port: 0u16,
            ws_port: 0u16,
            wss_port: 0u16,
            outbound_udpv4_protocol_handler: None,
            outbound_udpv6_protocol_handler: None,
            interfaces: NetworkInterfaces::new(),
        }
    }

    fn new_unlocked_inner() -> NetworkUnlockedInner {
        NetworkUnlockedInner {
            update_udpv4_dialinfo_task: TickTask::new(1),
            update_tcpv4_dialinfo_task: TickTask::new(1),
        }
    }

    pub fn new(network_manager: NetworkManager) -> Self {
        let this = Self {
            config: network_manager.config(),
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
            unlocked_inner: Arc::new(Self::new_unlocked_inner()),
        };

        // Set udp dialinfo tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .update_udpv4_dialinfo_task
                .set_routine(move |l, t| {
                    Box::pin(this2.clone().update_udpv4_dialinfo_task_routine(l, t))
                });
        }
        // Set tcp dialinfo tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .update_tcpv4_dialinfo_task
                .set_routine(move |l, t| {
                    Box::pin(this2.clone().update_tcpv4_dialinfo_task_routine(l, t))
                });
        }

        this
    }

    fn routing_table(&self) -> RoutingTable {
        self.inner.lock().routing_table.clone()
    }

    fn load_certs(path: &Path) -> io::Result<Vec<Certificate>> {
        let cvec = certs(&mut BufReader::new(File::open(path)?))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid TLS certificate"))?;
        Ok(cvec.into_iter().map(Certificate).collect())
    }

    fn load_keys(path: &Path) -> io::Result<Vec<PrivateKey>> {
        {
            if let Ok(v) = rsa_private_keys(&mut BufReader::new(File::open(path)?)) {
                if !v.is_empty() {
                    return Ok(v.into_iter().map(PrivateKey).collect());
                }
            }
        }
        {
            if let Ok(v) = pkcs8_private_keys(&mut BufReader::new(File::open(path)?)) {
                if !v.is_empty() {
                    return Ok(v.into_iter().map(PrivateKey).collect());
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid TLS private key",
        ))
    }

    fn load_server_config(&self) -> io::Result<ServerConfig> {
        let c = self.config.get();
        //
        trace!(
            "loading certificate from {}",
            c.network.tls.certificate_path
        );
        let certs = Self::load_certs(&PathBuf::from(&c.network.tls.certificate_path))?;
        trace!("loaded {} certificates", certs.len());
        if certs.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("Certificates at {} could not be loaded.\nEnsure it is in PEM format, beginning with '-----BEGIN CERTIFICATE-----'",c.network.tls.certificate_path)));
        }
        //
        trace!(
            "loading private key from {}",
            c.network.tls.private_key_path
        );
        let mut keys = Self::load_keys(&PathBuf::from(&c.network.tls.private_key_path))?;
        trace!("loaded {} keys", keys.len());
        if keys.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("Private key at {} could not be loaded.\nEnsure it is unencrypted and in RSA or PKCS8 format, beginning with '-----BEGIN RSA PRIVATE KEY-----' or '-----BEGIN PRIVATE KEY-----'",c.network.tls.private_key_path)));
        }

        // xxx: rustls ^0.20
        // let mut config = ServerConfig::builder()
        //     .with_safe_defaults()
        //     .with_no_client_auth()
        //     .with_single_cert(certs, keys.remove(0))
        //     .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let mut config = ServerConfig::new(NoClientAuth::new());
        config
            .set_single_cert(certs, keys.remove(0))
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        Ok(config)
    }

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

    fn add_to_join_handles(&self, jh: JoinHandle<()>) {
        let mut inner = self.inner.lock();
        inner.join_handles.push(jh);
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
    async fn start_tcp_listener(
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

    ////////////////////////////////////////////////////////////
    async fn create_udp_outbound_sockets(&self) -> Result<(), String> {
        let mut inner = self.inner.lock();
        let mut port = inner.udp_port;
        // v4
        let socket_addr_v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
        if let Ok(socket) = new_shared_udp_socket(socket_addr_v4) {
            log_net!("created udpv4 outbound socket on {:?}", &socket_addr_v4);

            // Pull the port if we randomly bound, so v6 can be on the same port
            port = socket
                .local_addr()
                .map_err(map_to_string)?
                .as_socket_ipv4()
                .ok_or_else(|| "expected ipv4 address type".to_owned())?
                .port();

            // Make an async UdpSocket from the socket2 socket
            let std_udp_socket: std::net::UdpSocket = socket.into();
            let udp_socket = UdpSocket::from(std_udp_socket);
            let socket_arc = Arc::new(udp_socket);

            // Create protocol handler
            let udpv4_handler =
                RawUdpProtocolHandler::new(inner.network_manager.clone(), socket_arc.clone());

            inner.outbound_udpv4_protocol_handler = Some(udpv4_handler);
        }
        //v6
        let socket_addr_v6 =
            SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), port);
        if let Ok(socket) = new_shared_udp_socket(socket_addr_v6) {
            log_net!("created udpv6 outbound socket on {:?}", &socket_addr_v6);

            // Make an async UdpSocket from the socket2 socket
            let std_udp_socket: std::net::UdpSocket = socket.into();
            let udp_socket = UdpSocket::from(std_udp_socket);
            let socket_arc = Arc::new(udp_socket);

            // Create protocol handler
            let udpv6_handler =
                RawUdpProtocolHandler::new(inner.network_manager.clone(), socket_arc.clone());

            inner.outbound_udpv6_protocol_handler = Some(udpv6_handler);
        }

        Ok(())
    }

    async fn spawn_udp_inbound_socket(&self, addr: SocketAddr) -> Result<(), String> {
        log_net!("spawn_udp_inbound_socket on {:?}", &addr);

        // Create a reusable socket
        let socket = new_shared_udp_socket(addr)?;

        // Make an async UdpSocket from the socket2 socket
        let std_udp_socket: std::net::UdpSocket = socket.into();
        let udp_socket = UdpSocket::from(std_udp_socket);
        let socket_arc = Arc::new(udp_socket);

        // Create protocol handler
        let protocol_handler = RawUdpProtocolHandler::new(
            self.inner.lock().network_manager.clone(),
            socket_arc.clone(),
        );

        // Create message_handler records
        self.inner
            .lock()
            .udp_protocol_handlers
            .insert(addr, protocol_handler.clone());

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
            let socket = socket_arc.clone();
            let protocol_handler = protocol_handler.clone();
            trace!("Spawning UDP listener task");

            ////////////////////////////////////////////////////////////
            // Run task for messages
            let this = self.clone();
            let jh = spawn(async move {
                trace!("UDP listener task spawned");

                let mut data = vec![0u8; 65536];
                while let Ok((size, socket_addr)) = socket.recv_from(&mut data).await {
                    // XXX: Limit the number of packets from the same IP address?
                    trace!("UDP packet from: {}", socket_addr);

                    let _processed = protocol_handler
                        .clone()
                        .on_message(&data[..size], socket_addr)
                        .await;
                }
                trace!("UDP listener task stopped");
                // If this loop fails, our socket died and we need to restart the network
                this.inner.lock().network_needs_restart = true;
            });
            ////////////////////////////////////////////////////////////

            // Add to join handle
            self.add_to_join_handles(jh);
        }

        Ok(())
    }

    fn translate_unspecified_address(inner: &NetworkInner, from: &SocketAddr) -> Vec<SocketAddr> {
        if !from.ip().is_unspecified() {
            vec![*from]
        } else {
            let mut out = Vec::<SocketAddr>::with_capacity(inner.interfaces.len());
            for (_, intf) in inner.interfaces.iter() {
                if intf.is_loopback() {
                    continue;
                }
                if let Some(pipv4) = intf.primary_ipv4() {
                    out.push(SocketAddr::new(IpAddr::V4(pipv4), from.port()));
                }
                if let Some(pipv6) = intf.primary_ipv6() {
                    out.push(SocketAddr::new(IpAddr::V6(pipv6), from.port()));
                }
            }
            out
        }
    }

    async fn start_udp_handler(&self, address: String) -> Result<Vec<DialInfo>, String> {
        let mut out = Vec::<DialInfo>::new();
        // convert to socketaddrs
        let mut sockaddrs = address
            .to_socket_addrs()
            .await
            .map_err(|e| format!("Unable to resolve address: {}\n{}", address, e))?;
        for addr in &mut sockaddrs {
            // see if we've already bound to this already
            // if not, spawn a listener
            if !self.inner.lock().udp_protocol_handlers.contains_key(&addr) {
                let ldi_addrs = Self::translate_unspecified_address(&*self.inner.lock(), &addr);

                self.clone().spawn_udp_inbound_socket(addr).await?;

                // Return local dial infos we listen on
                for ldi_addr in ldi_addrs {
                    out.push(DialInfo::udp_from_socketaddr(ldi_addr));
                }
            }
        }
        Ok(out)
    }

    /////////////////////////////////////////////////////////////////

    fn find_best_udp_protocol_handler(
        &self,
        peer_socket_addr: &SocketAddr,
        local_socket_addr: &Option<SocketAddr>,
    ) -> Option<RawUdpProtocolHandler> {
        // if our last communication with this peer came from a particular udp protocol handler, use it
        if let Some(sa) = local_socket_addr {
            if let Some(ph) = self.inner.lock().udp_protocol_handlers.get(sa) {
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

    fn get_preferred_local_address(
        &self,
        local_port: u16,
        peer_socket_addr: &SocketAddr,
    ) -> SocketAddr {
        match peer_socket_addr {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), local_port),
            SocketAddr::V6(_) => SocketAddr::new(
                IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
                local_port,
            ),
        }
    }

    async fn send_data_to_existing_connection(
        &self,
        descriptor: &ConnectionDescriptor,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        match descriptor.protocol_type() {
            ProtocolType::UDP => {
                // send over the best udp socket we have bound since UDP is not connection oriented
                let peer_socket_addr = descriptor.remote.to_socket_addr();
                if let Some(ph) = self.find_best_udp_protocol_handler(
                    &peer_socket_addr,
                    &descriptor.local.map(|sa| sa.to_socket_addr()),
                ) {
                    ph.clone()
                        .send_message(data, peer_socket_addr)
                        .await
                        .map_err(logthru_net!())?;
                    // Data was consumed
                    return Ok(None);
                }
            }
            ProtocolType::TCP | ProtocolType::WS | ProtocolType::WSS => {
                // find an existing connection in the connection table if one exists
                let network_manager = self.inner.lock().network_manager.clone();
                if let Some(entry) = network_manager
                    .connection_table()
                    .get_connection(descriptor)
                {
                    // connection exists, send over it
                    entry.conn.send(data).await.map_err(logthru_net!())?;

                    // Data was consumed
                    return Ok(None);
                }
            }
        }
        // connection or local socket didn't exist, we'll need to use dialinfo to create one
        // Pass the data back out so we don't own it any more
        Ok(Some(data))
    }

    pub async fn send_data_unbound_to_dial_info(
        &self,
        dial_info: &DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        match &dial_info {
            DialInfo::UDP(_) => {
                let peer_socket_addr = dial_info.to_socket_addr();
                RawUdpProtocolHandler::send_unbound_message(data, peer_socket_addr)
                    .await
                    .map_err(logthru_net!())
            }
            DialInfo::TCP(_) => {
                let peer_socket_addr = dial_info.to_socket_addr();
                RawTcpProtocolHandler::send_unbound_message(data, peer_socket_addr)
                    .await
                    .map_err(logthru_net!())
            }
            DialInfo::WS(_) => Err("WS protocol does not support unbound messages".to_owned())
                .map_err(logthru_net!(error)),
            DialInfo::WSS(_) => Err("WSS protocol does not support unbound messages".to_owned())
                .map_err(logthru_net!(error)),
        }
    }

    pub async fn send_data_to_dial_info(
        &self,
        dial_info: &DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let network_manager = self.inner.lock().network_manager.clone();

        let conn = match &dial_info {
            DialInfo::UDP(_) => {
                let peer_socket_addr = dial_info.to_socket_addr();
                if let Some(ph) = self.find_best_udp_protocol_handler(&peer_socket_addr, &None) {
                    return ph
                        .send_message(data, peer_socket_addr)
                        .await
                        .map_err(logthru_net!());
                } else {
                    return Err("no appropriate UDP protocol handler for dial_info".to_owned())
                        .map_err(logthru_net!(error));
                }
            }
            DialInfo::TCP(_) => {
                let peer_socket_addr = dial_info.to_socket_addr();
                let local_addr =
                    self.get_preferred_local_address(self.inner.lock().tcp_port, &peer_socket_addr);
                RawTcpProtocolHandler::connect(network_manager, local_addr, peer_socket_addr)
                    .await
                    .map_err(logthru_net!())?
            }
            DialInfo::WS(_) => {
                let peer_socket_addr = dial_info.to_socket_addr();
                let local_addr =
                    self.get_preferred_local_address(self.inner.lock().ws_port, &peer_socket_addr);
                WebsocketProtocolHandler::connect(network_manager, local_addr, dial_info)
                    .await
                    .map_err(logthru_net!(error))?
            }
            DialInfo::WSS(_) => {
                let peer_socket_addr = dial_info.to_socket_addr();
                let local_addr =
                    self.get_preferred_local_address(self.inner.lock().wss_port, &peer_socket_addr);
                WebsocketProtocolHandler::connect(network_manager, local_addr, dial_info)
                    .await
                    .map_err(logthru_net!(error))?
            }
        };

        conn.send(data).await.map_err(logthru_net!(error))
    }

    pub async fn send_data(&self, node_ref: NodeRef, data: Vec<u8>) -> Result<(), String> {
        let dial_info = node_ref.best_dial_info();
        let descriptor = node_ref.last_connection();

        // First try to send data to the last socket we've seen this peer on
        let di_data = if let Some(descriptor) = descriptor {
            match self
                .clone()
                .send_data_to_existing_connection(&descriptor, data)
                .await?
            {
                None => {
                    return Ok(());
                }
                Some(d) => d,
            }
        } else {
            data
        };

        // If that fails, try to make a connection or reach out to the peer via its dial info
        if let Some(di) = dial_info {
            self.clone().send_data_to_dial_info(&di, di_data).await
        } else {
            Err("couldn't send data, no dial info or peer address".to_owned())
        }
    }

    /////////////////////////////////////////////////////////////////

    /////////////////////////////////////////////////////////////////

    pub async fn start_udp_listeners(&self) -> Result<(), String> {
        let routing_table = self.routing_table();
        let (listen_address, public_address) = {
            let c = self.config.get();
            (
                c.network.protocol.udp.listen_address.clone(),
                c.network.protocol.udp.public_address.clone(),
            )
        };
        info!("UDP: starting listener at {:?}", listen_address);
        let dial_infos = self.start_udp_handler(listen_address.clone()).await?;
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
        Ok(())
    }

    pub async fn start_ws_listeners(&self) -> Result<(), String> {
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

    pub async fn start_wss_listeners(&self) -> Result<(), String> {
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

    pub async fn start_tcp_listeners(&self) -> Result<(), String> {
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

    pub fn get_protocol_config(&self) -> Option<ProtocolConfig> {
        self.inner.lock().protocol_config
    }

    pub async fn startup(&self) -> Result<(), String> {
        info!("starting network");

        // initialize interfaces
        self.inner.lock().interfaces.refresh()?;

        // get protocol config
        let protocol_config = {
            let c = self.config.get();
            ProtocolConfig {
                udp_enabled: c.network.protocol.udp.enabled && c.capabilities.protocol_udp,
                tcp_connect: c.network.protocol.tcp.connect && c.capabilities.protocol_connect_tcp,
                tcp_listen: c.network.protocol.tcp.listen && c.capabilities.protocol_accept_tcp,
                ws_connect: c.network.protocol.ws.connect && c.capabilities.protocol_connect_ws,
                ws_listen: c.network.protocol.ws.listen && c.capabilities.protocol_accept_ws,
                wss_connect: c.network.protocol.wss.connect && c.capabilities.protocol_connect_wss,
                wss_listen: c.network.protocol.wss.listen && c.capabilities.protocol_accept_wss,
            }
        };
        self.inner.lock().protocol_config = Some(protocol_config);

        // start listeners
        if protocol_config.udp_enabled {
            self.start_udp_listeners().await?;
            self.create_udp_outbound_sockets().await?;
        }
        if protocol_config.ws_listen {
            self.start_ws_listeners().await?;
        }
        if protocol_config.wss_listen {
            self.start_wss_listeners().await?;
        }
        if protocol_config.tcp_listen {
            self.start_tcp_listeners().await?;
        }

        info!("network started");
        self.inner.lock().network_started = true;
        Ok(())
    }

    pub fn needs_restart(&self) -> bool {
        self.inner.lock().network_needs_restart
    }

    pub async fn shutdown(&self) {
        info!("stopping network");

        // Reset state
        let network_manager = self.inner.lock().network_manager.clone();
        let routing_table = network_manager.routing_table();

        // Drop all dial info
        routing_table.clear_dial_info_details();

        // Cancels all async background tasks by dropping join handles
        *self.inner.lock() = Self::new_inner(network_manager);

        info!("network stopped");
    }

    //////////////////////////////////////////
    pub fn get_network_class(&self) -> Option<NetworkClass> {
        let inner = self.inner.lock();
        let routing_table = inner.routing_table.clone();

        if !inner.network_started {
            return None;
        }

        // If we've fixed the network class, return it rather than calculating it
        if inner.network_class.is_some() {
            return inner.network_class;
        }

        // Go through our global dialinfo and see what our best network class is
        let mut network_class = NetworkClass::Invalid;
        for did in routing_table.global_dial_info_details() {
            if let Some(nc) = did.network_class {
                if nc < network_class {
                    network_class = nc;
                }
            }
        }
        Some(network_class)
    }

    //////////////////////////////////////////

    pub async fn tick(&self) -> Result<(), String> {
        let (
            routing_table,
            protocol_config,
            udp_static_public_dialinfo,
            tcp_static_public_dialinfo,
            network_class,
        ) = {
            let inner = self.inner.lock();
            (
                inner.network_manager.routing_table(),
                inner.protocol_config.unwrap_or_default(),
                inner.udp_static_public_dialinfo,
                inner.tcp_static_public_dialinfo,
                inner.network_class.unwrap_or(NetworkClass::Invalid),
            )
        };

        // See if we have any UDPv4 public dialinfo, and if we should have it
        // If we have statically configured public dialinfo, don't bother with this
        // If we can have public dialinfo, or we haven't figured out our network class yet,
        // and we're active for UDP, we should attempt to get our public dialinfo sorted out
        // and assess our network class if we haven't already
        if protocol_config.udp_enabled
            && !udp_static_public_dialinfo
            && (network_class.inbound_capable() || network_class == NetworkClass::Invalid)
        {
            let filter = DialInfoFilter::global()
                .with_protocol_type(ProtocolType::UDP)
                .with_address_type(AddressType::IPV4);
            let need_udpv4_dialinfo = routing_table
                .first_filtered_dial_info_detail(&filter)
                .is_none();
            if need_udpv4_dialinfo {
                // If we have no public UDPv4 dialinfo, then we need to run a NAT check
                // ensure the singlefuture is running for this
                self.unlocked_inner
                    .update_udpv4_dialinfo_task
                    .tick()
                    .await?;
            }
        }

        // Same but for TCPv4
        if protocol_config.tcp_listen
            && !tcp_static_public_dialinfo
            && (network_class.inbound_capable() || network_class == NetworkClass::Invalid)
        {
            let filter = DialInfoFilter::global()
                .with_protocol_type(ProtocolType::TCP)
                .with_address_type(AddressType::IPV4);
            let need_tcpv4_dialinfo = routing_table
                .first_filtered_dial_info_detail(&filter)
                .is_none();
            if need_tcpv4_dialinfo {
                // If we have no public TCPv4 dialinfo, then we need to run a NAT check
                // ensure the singlefuture is running for this
                self.unlocked_inner
                    .update_tcpv4_dialinfo_task
                    .tick()
                    .await?;
            }
        }

        Ok(())
    }
}

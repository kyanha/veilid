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
use cfg_if::*;
use futures_util::StreamExt;
// xxx: rustls ^0.20
//use rustls::{server::NoClientAuth, Certificate, PrivateKey, ServerConfig};
use rustls::{Certificate, NoClientAuth, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use socket2::{Domain, Protocol, Socket, Type};
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
    network_needs_restart: bool,
    udp_listen: bool,
    udp_static_public_dialinfo: bool,
    tcp_listen: bool,
    tcp_static_public_dialinfo: bool,
    ws_listen: bool,
    wss_listen: bool,
    network_class: Option<NetworkClass>,
    join_handles: Vec<JoinHandle<()>>,
    listener_states: BTreeMap<SocketAddr, Arc<RwLock<ListenerState>>>,
    udp_protocol_handlers: BTreeMap<SocketAddr, RawUdpProtocolHandler>,
    tls_acceptor: Option<TlsAcceptor>,
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
            network_needs_restart: false,
            udp_listen: false,
            udp_static_public_dialinfo: false,
            tcp_listen: false,
            tcp_static_public_dialinfo: false,
            ws_listen: false,
            wss_listen: false,
            network_class: None,
            join_handles: Vec::new(),
            listener_states: BTreeMap::new(),
            udp_protocol_handlers: BTreeMap::new(),
            tls_acceptor: None,
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
        let domain = Domain::for_address(addr);
        let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
            .map_err(|e| format!("Couldn't create TCP socket: {}", e))?;
        if let Err(e) = socket.set_linger(None) {
            warn!("Couldn't set TCP linger: {}", e);
        }
        if let Err(e) = socket.set_nodelay(true) {
            warn!("Couldn't set TCP nodelay: {}", e);
        }
        if let Err(e) = socket.set_reuse_address(true) {
            warn!("Couldn't set reuse address: {}", e);
        }
        cfg_if! {
            if #[cfg(unix)] {
                if let Err(e) = socket.set_reuse_port(true) {
                    warn!("Couldn't set reuse port: {}", e);
                }
            }
        }

        // Bind a listener and stash it with the sockaddr in a table
        trace!("spawn_socket_listener: binding to {}", addr);
        let socket2_addr = socket2::SockAddr::from(addr);
        socket
            .bind(&socket2_addr)
            .map_err(|e| format!("failed to bind TCP socket: {}", e))?;

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

    async fn start_tcp_listener(
        &self,
        address: String,
        is_tls: bool,
        new_tcp_protocol_handler: Box<NewTcpProtocolHandler>,
    ) -> Result<Vec<(Address, u16)>, String> {
        let mut out = Vec::<(Address, u16)>::new();
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
                out.push((Address::from_socket_addr(ldi_addr), ldi_addr.port()));
            }
        }

        Ok(out)
    }

    ////////////////////////////////////////////////////////////
    async fn spawn_udp_socket(&self, addr: SocketAddr) -> Result<(), String> {
        trace!("spawn_udp_socket on {:?}", &addr);

        // Create a reusable socket
        let domain = Domain::for_address(addr);
        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|e| format!("Couldn't create UDP socket: {}", e))?;
        if let Err(e) = socket.set_reuse_address(true) {
            warn!("Couldn't set reuse address: {}", e);
        }
        cfg_if! {
            if #[cfg(unix)] {
                if let Err(e) = socket.set_reuse_port(true) {
                    warn!("Couldn't set reuse port: {}", e);
                }
            }
        }

        // Bind a listener and stash it with the sockaddr in a table
        trace!("spawn_udp_socket: binding to {}", addr);
        let socket2_addr = socket2::SockAddr::from(addr);
        socket
            .bind(&socket2_addr)
            .map_err(|e| format!("failed to bind UDP socket: {}", e))?;

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

                self.clone().spawn_udp_socket(addr).await?;

                // Return local dial infos we listen on
                for ldi_addr in ldi_addrs {
                    out.push(DialInfo::udp(
                        Address::from_socket_addr(ldi_addr),
                        ldi_addr.port(),
                    ));
                }
            }
        }
        Ok(out)
    }

    /////////////////////////////////////////////////////////////////

    fn match_socket_addr(
        inner: &NetworkInner,
        listen_socket_addr: &SocketAddr,
        peer_socket_addr: &SocketAddr,
    ) -> bool {
        let ldi_addrs = Self::translate_unspecified_address(inner, listen_socket_addr);
        // xxx POSSIBLE CONCERN (verify this?)
        // xxx will need to be reworked to search routing table information if we
        // xxx allow systems to be dual homed with multiple interfaces eventually
        // xxx to ensure the socket on the appropriate interface is chosen
        // xxx this may not be necessary if the kernel automatically picks the right interface
        // xxx it may do that. need to verify that though
        for local_addr in &ldi_addrs {
            if mem::discriminant(local_addr) == mem::discriminant(peer_socket_addr) {
                return true;
            }
        }

        false
    }

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
        // otherwise find the first udp protocol handler that matches the ip protocol version of the peer addr
        let inner = self.inner.lock();
        for (local_addr, ph) in &inner.udp_protocol_handlers {
            if Self::match_socket_addr(&*inner, local_addr, peer_socket_addr) {
                return Some(ph.clone());
            }
        }
        None
    }

    fn find_best_tcp_local_address(&self, peer_socket_addr: &SocketAddr) -> Option<SocketAddr> {
        // Find a matching listening local tcp socket address if we can
        let routing_table = self.routing_table();
        let dids = routing_table.local_dial_info_for_protocol(ProtocolType::TCP);
        let inner = self.inner.lock();
        for did in dids {
            if let Ok(local_addr) = did.dial_info.to_socket_addr() {
                if Self::match_socket_addr(&*inner, &local_addr, peer_socket_addr) {
                    return Some(local_addr);
                }
            }
        }
        None
    }

    async fn send_data_to_existing_connection(
        &self,
        descriptor: &ConnectionDescriptor,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        match descriptor.protocol_type() {
            ProtocolType::UDP => {
                // send over the best udp socket we have bound since UDP is not connection oriented
                let peer_socket_addr = descriptor
                    .remote
                    .to_socket_addr()
                    .map_err(|_| "unable to get socket address".to_owned())?;
                if let Some(ph) =
                    self.find_best_udp_protocol_handler(&peer_socket_addr, &descriptor.local)
                {
                    ph.clone()
                        .send_message(data, peer_socket_addr)
                        .await
                        .map_err(|_| "unable to send udp message".to_owned())?;
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
                    entry
                        .conn
                        .send(data)
                        .await
                        .map_err(|_| "failed to send tcp or ws message".to_owned())?;

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
                let peer_socket_addr = dial_info
                    .to_socket_addr()
                    .map_err(|_| "failed to resolve dial info for UDP dial info".to_owned())?;

                RawUdpProtocolHandler::send_unbound_message(data, peer_socket_addr)
                    .await
                    .map_err(|_| "failed to send unbound message to UDP dial info".to_owned())
            }
            DialInfo::TCP(_) => {
                let peer_socket_addr = dial_info
                    .to_socket_addr()
                    .map_err(|_| "failed to resolve dial info for TCP dial info".to_owned())?;
                RawTcpProtocolHandler::send_unbound_message(data, peer_socket_addr)
                    .await
                    .map_err(|_| "failed to connect to TCP dial info".to_owned())
            }
            DialInfo::WS(_) => Err("WS protocol does not support unbound messages".to_owned()),
            DialInfo::WSS(_) => Err("WSS protocol does not support unbound messages".to_owned()),
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
                let peer_socket_addr = dial_info
                    .to_socket_addr()
                    .map_err(|_| "failed to resolve dial info for UDP dial info".to_owned())?;
                if let Some(ph) = self.find_best_udp_protocol_handler(&peer_socket_addr, &None) {
                    return ph
                        .send_message(data, peer_socket_addr)
                        .await
                        .map_err(|_| "failed to send message to UDP dial info".to_owned());
                } else {
                    return Err("no appropriate udp protocol handler for dial_info".to_owned());
                }
            }
            DialInfo::TCP(_) => {
                let peer_socket_addr = dial_info
                    .to_socket_addr()
                    .map_err(|_| "failed to resolve dial info for TCP dial info".to_owned())?;
                let some_local_addr = self.find_best_tcp_local_address(&peer_socket_addr);
                RawTcpProtocolHandler::connect(network_manager, some_local_addr, peer_socket_addr)
                    .await
                    .map_err(|_| "failed to connect to TCP dial info".to_owned())?
            }
            DialInfo::WS(_) => WebsocketProtocolHandler::connect(network_manager, dial_info)
                .await
                .map_err(|_| "failed to connect to WS dial info".to_owned())?,
            DialInfo::WSS(_) => WebsocketProtocolHandler::connect(network_manager, dial_info)
                .await
                .map_err(|_| "failed to connect to WSS dial info".to_owned())?,
        };

        conn.send(data)
            .await
            .map_err(|_| "failed to send data to dial info".to_owned())
    }

    pub async fn send_data(&self, node_ref: NodeRef, data: Vec<u8>) -> Result<(), String> {
        let dial_info = node_ref.dial_info();
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
        trace!("UDP: starting listener at {:?}", listen_address);
        let dial_infos = self.start_udp_handler(listen_address.clone()).await?;
        trace!("UDP: listener started");

        for x in &dial_infos {
            routing_table.register_local_dial_info(x.clone(), DialInfoOrigin::Static);
        }

        // Add static public dialinfo if it's configured
        let mut static_public = false;
        if let Some(public_address) = public_address.as_ref() {
            // Resolve statically configured public dialinfo
            let mut public_sockaddrs = public_address
                .to_socket_addrs()
                .await
                .map_err(|e| format!("Unable to resolve address: {}\n{}", public_address, e))?;

            // Add all resolved addresses as public dialinfo
            for pdi_addr in &mut public_sockaddrs {
                routing_table.register_global_dial_info(
                    DialInfo::udp_from_socketaddr(pdi_addr),
                    Some(NetworkClass::Server),
                    DialInfoOrigin::Static,
                );
                static_public = true;
            }
        } else {
            // Register local dial info as public if it is publicly routable
            for x in &dial_infos {
                if x.is_public().unwrap_or(false) {
                    routing_table.register_global_dial_info(
                        x.clone(),
                        Some(NetworkClass::Server),
                        DialInfoOrigin::Static,
                    );
                    static_public = true;
                }
            }
        }
        self.inner.lock().udp_listen = true;
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
        let (host, port) = split_port(&listen_address)
            .map_err(|_| "invalid WS listen address, port not specified correctly".to_owned())?;
        let port = port.ok_or_else(|| "port must be specified for WS address".to_owned())?;
        let addresses = self
            .start_tcp_listener(
                listen_address.clone(),
                false,
                Box::new(|n, t, a| Box::new(WebsocketProtocolHandler::new(n, t, a))),
            )
            .await?;
        trace!("WS: listener started");

        let mut dial_infos: Vec<DialInfo> = Vec::new();
        for (a, p) in addresses {
            let di = DialInfo::ws(a.address_string(), p, path.clone());
            dial_infos.push(di.clone());
            routing_table.register_local_dial_info(di, DialInfoOrigin::Static);
        }

        // Add static public dialinfo if it's configured
        if let Some(url) = url.as_ref() {
            let split_url = SplitUrl::from_str(url)?;
            if split_url.scheme.to_ascii_lowercase() != "ws" {
                return Err("WS URL must use 'ws://' scheme".to_owned());
            }
            routing_table.register_global_dial_info(
                DialInfo::ws(
                    split_url.host,
                    split_url.port.unwrap_or(80),
                    split_url
                        .path
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "/".to_string()),
                ),
                Some(NetworkClass::Server),
                DialInfoOrigin::Static,
            );
        }

        self.inner.lock().ws_listen = true;
        Ok(())
    }

    pub async fn start_wss_listeners(&self) -> Result<(), String> {
        let routing_table = self.routing_table();
        let (listen_address, url, path) = {
            let c = self.config.get();
            (
                c.network.protocol.wss.listen_address.clone(),
                c.network.protocol.wss.url.clone(),
                c.network.protocol.wss.path.clone(),
            )
        };
        trace!("WSS: starting listener at {}", listen_address);
        let (host, port) = split_port(&listen_address)
            .map_err(|_| "invalid WSS listen address, port not specified correctly".to_owned())?;
        let port = port.ok_or_else(|| "port must be specified for WSS address".to_owned())?;

        let _ = self
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
        // let mut dial_infos: Vec<DialInfo> = Vec::new();
        // for (a, p) in addresses {
        //     let di = DialInfo::wss(a.address_string(), p, path.clone());
        //     dial_infos.push(di.clone());
        //     routing_table.register_local_dial_info(di, DialInfoOrigin::Static);
        // }

        // Add static public dialinfo if it's configured
        if let Some(url) = url.as_ref() {
            let split_url = SplitUrl::from_str(url)?;
            if split_url.scheme.to_ascii_lowercase() != "wss" {
                return Err("WSS URL must use 'wss://' scheme".to_owned());
            }
            routing_table.register_global_dial_info(
                DialInfo::wss(
                    split_url.host,
                    split_url.port.unwrap_or(443),
                    split_url
                        .path
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "/".to_string()),
                ),
                Some(NetworkClass::Server),
                DialInfoOrigin::Static,
            );
        } else {
            return Err("WSS URL must be specified due to TLS requirements".to_owned());
        }

        self.inner.lock().wss_listen = true;
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
        let addresses = self
            .start_tcp_listener(
                listen_address.clone(),
                false,
                Box::new(|n, _, a| Box::new(RawTcpProtocolHandler::new(n, a))),
            )
            .await?;
        trace!("TCP: listener started");

        let mut dial_infos: Vec<DialInfo> = Vec::new();
        for (a, p) in addresses {
            let di = DialInfo::tcp(a, p);
            dial_infos.push(di.clone());
            routing_table.register_local_dial_info(di, DialInfoOrigin::Static);
        }

        // Add static public dialinfo if it's configured
        let mut static_public = false;
        if let Some(public_address) = public_address.as_ref() {
            // Resolve statically configured public dialinfo
            let mut public_sockaddrs = public_address
                .to_socket_addrs()
                .await
                .map_err(|e| format!("Unable to resolve address: {}\n{}", public_address, e))?;

            // Add all resolved addresses as public dialinfo
            for pdi_addr in &mut public_sockaddrs {
                routing_table.register_global_dial_info(
                    DialInfo::tcp_from_socketaddr(pdi_addr),
                    None,
                    DialInfoOrigin::Static,
                );
                static_public = true;
            }
        } else {
            // Register local dial info as public if it is publicly routable
            for x in &dial_infos {
                if x.is_public().unwrap_or(false) {
                    routing_table.register_global_dial_info(
                        x.clone(),
                        Some(NetworkClass::Server),
                        DialInfoOrigin::Static,
                    );
                    static_public = true;
                }
            }
        }

        self.inner.lock().tcp_listen = true;
        self.inner.lock().tcp_static_public_dialinfo = static_public;

        Ok(())
    }

    pub async fn startup(&self) -> Result<(), String> {
        info!("starting network");

        // initialize interfaces
        self.inner.lock().interfaces.refresh()?;

        // get listen config
        let (listen_udp, listen_tcp, listen_ws, listen_wss) = {
            let c = self.config.get();
            (
                c.network.protocol.udp.enabled && c.capabilities.protocol_udp,
                c.network.protocol.tcp.listen && c.capabilities.protocol_accept_tcp,
                c.network.protocol.ws.listen && c.capabilities.protocol_accept_ws,
                c.network.protocol.wss.listen && c.capabilities.protocol_accept_wss,
            )
        };

        // start listeners
        if listen_udp {
            self.start_udp_listeners().await?;
        }
        if listen_ws {
            self.start_ws_listeners().await?;
        }
        if listen_wss {
            self.start_wss_listeners().await?;
        }
        if listen_tcp {
            self.start_tcp_listeners().await?;
        }

        info!("network started");
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
        routing_table.clear_local_dial_info();
        routing_table.clear_global_dial_info();

        // Cancels all async background tasks by dropping join handles
        *self.inner.lock() = Self::new_inner(network_manager);

        info!("network stopped");
    }

    //////////////////////////////////////////
    pub fn get_network_class(&self) -> NetworkClass {
        let inner = self.inner.lock();
        let routing_table = inner.routing_table.clone();

        // If we've fixed the network class, return it rather than calculating it
        if let Some(network_class) = inner.network_class {
            return network_class;
        }

        // Go through our public dialinfo and see what our best network class is
        let mut network_class = NetworkClass::Invalid;
        for x in routing_table.global_dial_info() {
            if let Some(nc) = x.network_class {
                if nc < network_class {
                    network_class = nc;
                }
            }
        }
        network_class
    }

    //////////////////////////////////////////

    pub async fn tick(&self) -> Result<(), String> {
        let (
            routing_table,
            udp_listen,
            udp_static_public_dialinfo,
            tcp_listen,
            tcp_static_public_dialinfo,
            network_class,
        ) = {
            let inner = self.inner.lock();
            (
                inner.network_manager.routing_table(),
                inner.udp_listen,
                inner.udp_static_public_dialinfo,
                inner.tcp_listen,
                inner.tcp_static_public_dialinfo,
                inner.network_class.unwrap_or(NetworkClass::Invalid),
            )
        };

        // See if we have any UDPv4 public dialinfo, and if we should have it
        // If we have statically configured public dialinfo, don't bother with this
        // If we can have public dialinfo, or we haven't figured out our network class yet,
        // and we're active for UDP, we should attempt to get our public dialinfo sorted out
        // and assess our network class if we haven't already
        if udp_listen
            && !udp_static_public_dialinfo
            && (network_class.inbound_capable() || network_class == NetworkClass::Invalid)
        {
            let need_udpv4_dialinfo = routing_table
                .global_dial_info_for_protocol_address_type(ProtocolAddressType::UDPv4)
                .is_empty();
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
        if tcp_listen
            && !tcp_static_public_dialinfo
            && (network_class.inbound_capable() || network_class == NetworkClass::Invalid)
        {
            let need_tcpv4_dialinfo = routing_table
                .global_dial_info_for_protocol_address_type(ProtocolAddressType::TCPv4)
                .is_empty();
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

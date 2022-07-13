mod network_class_discovery;
mod network_tcp;
mod network_udp;
mod protocol;
mod start_protocols;

use super::*;
use crate::routing_table::*;
use connection_manager::*;
use network_tcp::*;
use protocol::tcp::RawTcpProtocolHandler;
use protocol::udp::RawUdpProtocolHandler;
use protocol::ws::WebsocketProtocolHandler;
pub use protocol::*;
use utils::network_interfaces::*;

use async_tls::TlsAcceptor;
use futures_util::StreamExt;
use std::io;
// xxx: rustls ^0.20
//use rustls::{server::NoClientAuth, Certificate, PrivateKey, ServerConfig};
use rustls::{Certificate, NoClientAuth, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/////////////////////////////////////////////////////////////////

pub const PEEK_DETECT_LEN: usize = 64;

/////////////////////////////////////////////////////////////////

struct NetworkInner {
    network_started: bool,
    network_needs_restart: bool,
    protocol_config: Option<ProtocolConfig>,
    static_public_dialinfo: ProtocolSet,
    network_class: Option<NetworkClass>,
    join_handles: Vec<MustJoinHandle<()>>,
    stop_source: Option<StopSource>,
    udp_port: u16,
    tcp_port: u16,
    ws_port: u16,
    wss_port: u16,
    interfaces: NetworkInterfaces,
    // udp
    bound_first_udp: BTreeMap<u16, Option<(socket2::Socket, socket2::Socket)>>,
    inbound_udp_protocol_handlers: BTreeMap<SocketAddr, RawUdpProtocolHandler>,
    outbound_udpv4_protocol_handler: Option<RawUdpProtocolHandler>,
    outbound_udpv6_protocol_handler: Option<RawUdpProtocolHandler>,
    //tcp
    bound_first_tcp: BTreeMap<u16, Option<(socket2::Socket, socket2::Socket)>>,
    tls_acceptor: Option<TlsAcceptor>,
    listener_states: BTreeMap<SocketAddr, Arc<RwLock<ListenerState>>>,
}

struct NetworkUnlockedInner {
    // Accessors
    routing_table: RoutingTable,
    network_manager: NetworkManager,
    connection_manager: ConnectionManager,
    // Background processes
    update_network_class_task: TickTask<EyreReport>,
}

#[derive(Clone)]
pub struct Network {
    config: VeilidConfig,
    inner: Arc<Mutex<NetworkInner>>,
    unlocked_inner: Arc<NetworkUnlockedInner>,
}

impl Network {
    fn new_inner() -> NetworkInner {
        NetworkInner {
            network_started: false,
            network_needs_restart: false,
            protocol_config: None,
            static_public_dialinfo: ProtocolSet::empty(),
            network_class: None,
            join_handles: Vec::new(),
            stop_source: None,
            udp_port: 0u16,
            tcp_port: 0u16,
            ws_port: 0u16,
            wss_port: 0u16,
            interfaces: NetworkInterfaces::new(),
            bound_first_udp: BTreeMap::new(),
            inbound_udp_protocol_handlers: BTreeMap::new(),
            outbound_udpv4_protocol_handler: None,
            outbound_udpv6_protocol_handler: None,
            bound_first_tcp: BTreeMap::new(),
            tls_acceptor: None,
            listener_states: BTreeMap::new(),
        }
    }

    fn new_unlocked_inner(
        network_manager: NetworkManager,
        routing_table: RoutingTable,
        connection_manager: ConnectionManager,
    ) -> NetworkUnlockedInner {
        NetworkUnlockedInner {
            network_manager,
            routing_table,
            connection_manager,
            update_network_class_task: TickTask::new(1),
        }
    }

    pub fn new(
        network_manager: NetworkManager,
        routing_table: RoutingTable,
        connection_manager: ConnectionManager,
    ) -> Self {
        let this = Self {
            config: network_manager.config(),
            inner: Arc::new(Mutex::new(Self::new_inner())),
            unlocked_inner: Arc::new(Self::new_unlocked_inner(
                network_manager,
                routing_table,
                connection_manager,
            )),
        };

        // Set update network class tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .update_network_class_task
                .set_routine(move |s, l, t| {
                    Box::pin(this2.clone().update_network_class_task_routine(s, l, t))
                });
        }

        this
    }

    fn network_manager(&self) -> NetworkManager {
        self.unlocked_inner.network_manager.clone()
    }

    fn routing_table(&self) -> RoutingTable {
        self.unlocked_inner.routing_table.clone()
    }

    fn connection_manager(&self) -> ConnectionManager {
        self.unlocked_inner.connection_manager.clone()
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

    fn add_to_join_handles(&self, jh: MustJoinHandle<()>) {
        let mut inner = self.inner.lock();
        inner.join_handles.push(jh);
    }

    fn translate_unspecified_address(inner: &NetworkInner, from: &SocketAddr) -> Vec<SocketAddr> {
        if !from.ip().is_unspecified() {
            vec![*from]
        } else {
            inner
                .interfaces
                .best_addresses()
                .iter()
                .filter_map(|a| {
                    // We create sockets that are only ipv6 or ipv6 (not dual, so only translate matching unspecified address)
                    if (a.is_ipv4() && from.is_ipv4()) || (a.is_ipv6() && from.is_ipv6()) {
                        Some(SocketAddr::new(*a, from.port()))
                    } else {
                        None
                    }
                })
                .collect()
        }
    }

    fn get_preferred_local_address(&self, dial_info: &DialInfo) -> SocketAddr {
        let inner = self.inner.lock();

        let local_port = match dial_info.protocol_type() {
            ProtocolType::UDP => inner.udp_port,
            ProtocolType::TCP => inner.tcp_port,
            ProtocolType::WS => inner.ws_port,
            ProtocolType::WSS => inner.wss_port,
        };

        match dial_info.address_type() {
            AddressType::IPV4 => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), local_port),
            AddressType::IPV6 => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), local_port),
        }
    }

    pub fn with_interface_addresses<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[IpAddr]) -> R,
    {
        let inner = self.inner.lock();
        inner.interfaces.with_best_addresses(f)
    }

    // See if our interface addresses have changed, if so we need to punt the network
    // and redo all our addresses. This is overkill, but anything more accurate
    // would require inspection of routing tables that we dont want to bother with
    pub async fn check_interface_addresses(&self) -> EyreResult<bool> {
        let mut inner = self.inner.lock();
        if !inner.interfaces.refresh().await? {
            return Ok(false);
        }
        inner.network_needs_restart = true;
        Ok(true)
    }

    ////////////////////////////////////////////////////////////

    // Send data to a dial info, unbound, using a new connection from a random port
    // This creates a short-lived connection in the case of connection-oriented protocols
    // for the purpose of sending this one message.
    // This bypasses the connection table as it is not a 'node to node' connection.
    #[instrument(level="trace", err, skip(self, data), fields(data.len = data.len()))]
    pub async fn send_data_unbound_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> EyreResult<()> {
        let data_len = data.len();
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                let h = RawUdpProtocolHandler::new_unspecified_bound_handler(&peer_socket_addr)
                    .await
                    .wrap_err("create socket failure")?;
                h.send_message(data, peer_socket_addr)
                    .await
                    .wrap_err("send message failure")?;
            }
            ProtocolType::TCP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                let pnc = RawTcpProtocolHandler::connect(None, peer_socket_addr)
                    .await
                    .wrap_err("connect failure")?;
                pnc.send(data).await.wrap_err("send failure")?;
            }
            ProtocolType::WS | ProtocolType::WSS => {
                let pnc = WebsocketProtocolHandler::connect(None, &dial_info)
                    .await
                    .wrap_err("connect failure")?;
                pnc.send(data).await.wrap_err("send failure")?;
            }
        }
        // Network accounting
        self.network_manager()
            .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);

        Ok(())
    }

    // Send data to a dial info, unbound, using a new connection from a random port
    // Waits for a specified amount of time to receive a single response
    // This creates a short-lived connection in the case of connection-oriented protocols
    // for the purpose of sending this one message.
    // This bypasses the connection table as it is not a 'node to node' connection.
    #[instrument(level="trace", err, skip(self, data), fields(ret.timeout_or, data.len = data.len()))]
    pub async fn send_recv_data_unbound_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
        timeout_ms: u32,
    ) -> EyreResult<TimeoutOr<Vec<u8>>> {
        let data_len = data.len();
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                let h = RawUdpProtocolHandler::new_unspecified_bound_handler(&peer_socket_addr)
                    .await
                    .wrap_err("create socket failure")?;
                h.send_message(data, peer_socket_addr)
                    .await
                    .wrap_err("send message failure")?;
                self.network_manager()
                    .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);

                // receive single response
                let mut out = vec![0u8; MAX_MESSAGE_SIZE];
                let timeout_or_ret = timeout(timeout_ms, h.recv_message(&mut out))
                    .await
                    .into_timeout_or()
                    .into_result()
                    .wrap_err("recv_message failure")?;
                let (recv_len, recv_addr) = match timeout_or_ret {
                    TimeoutOr::Value(v) => v,
                    TimeoutOr::Timeout => {
                        tracing::Span::current().record("ret.timeout_or", &"Timeout".to_owned());
                        return Ok(TimeoutOr::Timeout);
                    }
                };

                let recv_socket_addr = recv_addr.remote_address().to_socket_addr();
                self.network_manager()
                    .stats_packet_rcvd(recv_socket_addr.ip(), recv_len as u64);

                // if the from address is not the same as the one we sent to, then drop this
                if recv_socket_addr != peer_socket_addr {
                    bail!("wrong address");
                }
                out.resize(recv_len, 0u8);
                Ok(TimeoutOr::Value(out))
            }
            ProtocolType::TCP | ProtocolType::WS | ProtocolType::WSS => {
                let pnc = match dial_info.protocol_type() {
                    ProtocolType::UDP => unreachable!(),
                    ProtocolType::TCP => {
                        let peer_socket_addr = dial_info.to_socket_addr();
                        RawTcpProtocolHandler::connect(None, peer_socket_addr)
                            .await
                            .wrap_err("connect failure")?
                    }
                    ProtocolType::WS | ProtocolType::WSS => {
                        WebsocketProtocolHandler::connect(None, &dial_info)
                            .await
                            .wrap_err("connect failure")?
                    }
                };

                pnc.send(data).await.wrap_err("send failure")?;
                self.network_manager()
                    .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);

                let out = timeout(timeout_ms, pnc.recv())
                    .await
                    .into_timeout_or()
                    .into_result()
                    .wrap_err("recv failure")?;

                tracing::Span::current().record(
                    "ret.timeout_or",
                    &match out {
                        TimeoutOr::<Vec<u8>>::Value(ref v) => format!("Value(len={})", v.len()),
                        TimeoutOr::<Vec<u8>>::Timeout => "Timeout".to_owned(),
                    },
                );

                if let TimeoutOr::Value(out) = &out {
                    self.network_manager()
                        .stats_packet_rcvd(dial_info.to_ip_addr(), out.len() as u64);
                }

                Ok(out)
            }
        }
    }

    #[instrument(level="trace", err, skip(self, data), fields(data.len = data.len()))]
    pub async fn send_data_to_existing_connection(
        &self,
        descriptor: ConnectionDescriptor,
        data: Vec<u8>,
    ) -> EyreResult<Option<Vec<u8>>> {
        let data_len = data.len();

        // Handle connectionless protocol
        if descriptor.protocol_type() == ProtocolType::UDP {
            // send over the best udp socket we have bound since UDP is not connection oriented
            let peer_socket_addr = descriptor.remote().to_socket_addr();
            if let Some(ph) = self.find_best_udp_protocol_handler(
                &peer_socket_addr,
                &descriptor.local().map(|sa| sa.to_socket_addr()),
            ) {
                ph.clone()
                    .send_message(data, peer_socket_addr)
                    .await
                    .wrap_err("sending data to existing conection")?;

                // Network accounting
                self.network_manager()
                    .stats_packet_sent(peer_socket_addr.ip(), data_len as u64);

                // Data was consumed
                return Ok(None);
            }
        }

        // Handle connection-oriented protocols

        // Try to send to the exact existing connection if one exists
        if let Some(conn) = self.connection_manager().get_connection(descriptor).await {
            // connection exists, send over it
            conn.send_async(data)
                .await
                .wrap_err("sending data to existing connection")?;

            // Network accounting
            self.network_manager()
                .stats_packet_sent(descriptor.remote().to_socket_addr().ip(), data_len as u64);

            // Data was consumed
            Ok(None)
        } else {
            // Connection or didn't exist
            // Pass the data back out so we don't own it any more
            Ok(Some(data))
        }
    }

    // Send data directly to a dial info, possibly without knowing which node it is going to
    #[instrument(level="trace", err, skip(self, data), fields(data.len = data.len()))]
    pub async fn send_data_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> EyreResult<()> {
        let data_len = data.len();
        // Handle connectionless protocol
        if dial_info.protocol_type() == ProtocolType::UDP {
            let peer_socket_addr = dial_info.to_socket_addr();
            if let Some(ph) = self.find_best_udp_protocol_handler(&peer_socket_addr, &None) {
                let res = ph
                    .send_message(data, peer_socket_addr)
                    .await
                    .wrap_err("failed to send data to dial info");
                if res.is_ok() {
                    // Network accounting
                    self.network_manager()
                        .stats_packet_sent(peer_socket_addr.ip(), data_len as u64);
                }
                return res;
            }
            bail!("no appropriate UDP protocol handler for dial_info");
        }

        // Handle connection-oriented protocols
        let local_addr = self.get_preferred_local_address(&dial_info);
        let conn = self
            .connection_manager()
            .get_or_create_connection(Some(local_addr), dial_info.clone())
            .await?;

        let res = conn.send_async(data).await;
        if res.is_ok() {
            // Network accounting
            self.network_manager()
                .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);
        }
        res
    }

    /////////////////////////////////////////////////////////////////

    pub fn get_protocol_config(&self) -> Option<ProtocolConfig> {
        self.inner.lock().protocol_config
    }

    #[instrument(level = "debug", err, skip_all)]
    pub async fn startup(&self) -> EyreResult<()> {
        // initialize interfaces
        let mut interfaces = NetworkInterfaces::new();
        interfaces.refresh().await?;

        let protocol_config = {
            let mut inner = self.inner.lock();

            // Create stop source
            inner.stop_source = Some(StopSource::new());
            inner.interfaces = interfaces;

            // get protocol config
            let protocol_config = {
                let c = self.config.get();
                let mut inbound = ProtocolSet::new();

                if c.network.protocol.udp.enabled && c.capabilities.protocol_udp {
                    inbound.insert(ProtocolType::UDP);
                }
                if c.network.protocol.tcp.listen && c.capabilities.protocol_accept_tcp {
                    inbound.insert(ProtocolType::TCP);
                }
                if c.network.protocol.ws.listen && c.capabilities.protocol_accept_ws {
                    inbound.insert(ProtocolType::WS);
                }
                if c.network.protocol.wss.listen && c.capabilities.protocol_accept_wss {
                    inbound.insert(ProtocolType::WSS);
                }

                let mut outbound = ProtocolSet::new();
                if c.network.protocol.udp.enabled && c.capabilities.protocol_udp {
                    outbound.insert(ProtocolType::UDP);
                }
                if c.network.protocol.tcp.connect && c.capabilities.protocol_connect_tcp {
                    outbound.insert(ProtocolType::TCP);
                }
                if c.network.protocol.ws.connect && c.capabilities.protocol_connect_ws {
                    outbound.insert(ProtocolType::WS);
                }
                if c.network.protocol.wss.connect && c.capabilities.protocol_connect_wss {
                    outbound.insert(ProtocolType::WSS);
                }

                ProtocolConfig { inbound, outbound }
            };
            inner.protocol_config = Some(protocol_config);
            protocol_config
        };

        // start listeners
        if protocol_config.inbound.contains(ProtocolType::UDP) {
            self.start_udp_listeners().await?;
        }
        if protocol_config.inbound.contains(ProtocolType::WS) {
            self.start_ws_listeners().await?;
        }
        if protocol_config.inbound.contains(ProtocolType::WSS) {
            self.start_wss_listeners().await?;
        }
        if protocol_config.inbound.contains(ProtocolType::TCP) {
            self.start_tcp_listeners().await?;
        }

        // release caches of available listener ports
        // this releases the 'first bound' ports we use to guarantee
        // that we have ports available to us
        self.free_bound_first_ports();

        // If we have static public dialinfo, upgrade our network class
        {
            let mut inner = self.inner.lock();
            if !inner.static_public_dialinfo.is_empty() {
                inner.network_class = Some(NetworkClass::InboundCapable);
            }
        }

        info!("network started");
        self.inner.lock().network_started = true;

        // Inform routing table entries that our dial info has changed
        self.routing_table().send_node_info_updates(true).await;

        Ok(())
    }

    pub fn needs_restart(&self) -> bool {
        self.inner.lock().network_needs_restart
    }

    pub fn is_started(&self) -> bool {
        self.inner.lock().network_started
    }

    #[instrument(level = "debug", skip_all)]
    pub fn restart_network(&self) {
        self.inner.lock().network_needs_restart = true;
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn shutdown(&self) {
        debug!("starting low level network shutdown");

        let routing_table = self.routing_table();

        // Stop all tasks
        debug!("stopping update network class task");
        if let Err(e) = self.unlocked_inner.update_network_class_task.stop().await {
            error!("update_network_class_task not cancelled: {}", e);
        }

        let mut unord = FuturesUnordered::new();
        {
            let mut inner = self.inner.lock();
            // take the join handles out
            for h in inner.join_handles.drain(..) {
                trace!("joining: {:?}", h);
                unord.push(h);
            }
            // Drop the stop
            drop(inner.stop_source.take());
        }
        debug!("stopping {} low level network tasks", unord.len());
        // Wait for everything to stop
        while unord.next().await.is_some() {}

        debug!("clearing dial info");
        // Drop all dial info
        routing_table.clear_dial_info_details(RoutingDomain::PublicInternet);
        routing_table.clear_dial_info_details(RoutingDomain::LocalNetwork);

        // Reset state including network class
        *self.inner.lock() = Self::new_inner();

        debug!("finished low level network shutdown");
    }

    //////////////////////////////////////////
    pub fn get_network_class(&self) -> Option<NetworkClass> {
        let inner = self.inner.lock();
        inner.network_class
    }

    #[instrument(level = "debug", skip_all)]
    pub fn reset_network_class(&self) {
        let mut inner = self.inner.lock();
        inner.network_class = None;
    }

    //////////////////////////////////////////

    pub async fn tick(&self) -> EyreResult<()> {
        let network_class = self.get_network_class().unwrap_or(NetworkClass::Invalid);
        let routing_table = self.routing_table();

        // If we need to figure out our network class, tick the task for it
        if network_class == NetworkClass::Invalid {
            let rth = routing_table.get_routing_table_health();

            // Need at least two entries to do this
            if rth.unreliable_entry_count + rth.reliable_entry_count >= 2 {
                self.unlocked_inner.update_network_class_task.tick().await?;
            }
        }

        Ok(())
    }
}

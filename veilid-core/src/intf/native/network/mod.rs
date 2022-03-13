mod network_tcp;
mod network_udp;
mod protocol;
mod public_dialinfo_discovery;
mod start_protocols;

use crate::connection_manager::*;
use crate::intf::*;
use crate::network_manager::*;
use crate::routing_table::*;
use crate::*;
use network_tcp::*;
use protocol::tcp::RawTcpProtocolHandler;
use protocol::udp::RawUdpProtocolHandler;
use protocol::ws::WebsocketProtocolHandler;
pub use protocol::*;
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

    fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }

    fn routing_table(&self) -> RoutingTable {
        self.inner.lock().routing_table.clone()
    }

    fn connection_manager(&self) -> ConnectionManager {
        self.inner.lock().network_manager.connection_manager()
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

    fn add_to_join_handles(&self, jh: JoinHandle<()>) {
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
                .map(|a| SocketAddr::new(*a, from.port()))
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

    ////////////////////////////////////////////////////////////

    // Send data to a dial info, unbound, using a new connection from a random port
    // This creates a short-lived connection in the case of connection-oriented protocols
    // for the purpose of sending this one message.
    // This bypasses the connection table as it is not a 'node to node' connection.
    pub async fn send_data_unbound_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                RawUdpProtocolHandler::send_unbound_message(peer_socket_addr, data)
                    .await
                    .map_err(logthru_net!())
            }
            ProtocolType::TCP => {
                let peer_socket_addr = dial_info.to_socket_addr();
                RawTcpProtocolHandler::send_unbound_message(peer_socket_addr, data)
                    .await
                    .map_err(logthru_net!())
            }
            ProtocolType::WS | ProtocolType::WSS => {
                WebsocketProtocolHandler::send_unbound_message(dial_info, data)
                    .await
                    .map_err(logthru_net!())
            }
        }
    }

    async fn send_data_to_existing_connection(
        &self,
        descriptor: ConnectionDescriptor,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        // Handle connectionless protocol
        if descriptor.protocol_type() == ProtocolType::UDP {
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

        // Handle connection-oriented protocols

        // Try to send to the exact existing connection if one exists
        if let Some(conn) = self.connection_manager().get_connection(descriptor).await {
            // connection exists, send over it
            conn.send(data).await.map_err(logthru_net!())?;

            // Data was consumed
            Ok(None)
        } else {
            // Connection or didn't exist
            // Pass the data back out so we don't own it any more
            Ok(Some(data))
        }
    }

    // Send data directly to a dial info, possibly without knowing which node it is going to
    pub async fn send_data_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        // Handle connectionless protocol
        if dial_info.protocol_type() == ProtocolType::UDP {
            let peer_socket_addr = dial_info.to_socket_addr();
            if let Some(ph) = self.find_best_udp_protocol_handler(&peer_socket_addr, &None) {
                return ph
                    .send_message(data, peer_socket_addr)
                    .await
                    .map_err(logthru_net!());
            }
            return Err("no appropriate UDP protocol handler for dial_info".to_owned())
                .map_err(logthru_net!(error));
        }

        // Handle connection-oriented protocols
        let local_addr = self.get_preferred_local_address(&dial_info);
        let conn = self
            .connection_manager()
            .get_or_create_connection(Some(local_addr), dial_info)
            .await?;

        conn.send(data).await.map_err(logthru_net!(error))
    }

    // Send data to node
    // We may not have dial info for a node, but have an existing connection for it
    // because an inbound connection happened first, and no FindNodeQ has happened to that
    // node yet to discover its dial info. The existing connection should be tried first
    // in this case.
    pub async fn send_data(&self, node_ref: NodeRef, data: Vec<u8>) -> Result<(), String> {
        // First try to send data to the last socket we've seen this peer on
        let data = if let Some(descriptor) = node_ref.last_connection() {
            match self
                .clone()
                .send_data_to_existing_connection(descriptor, data)
                .await
                .map_err(logthru_net!())?
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
        let dial_info = node_ref
            .best_dial_info()
            .ok_or_else(|| "couldn't send data, no dial info or peer address".to_owned())?;

        self.send_data_to_dial_info(dial_info, data)
            .await
            .map_err(logthru_net!())
    }

    /////////////////////////////////////////////////////////////////

    pub fn get_protocol_config(&self) -> Option<ProtocolConfig> {
        self.inner.lock().protocol_config
    }

    pub async fn startup(&self) -> Result<(), String> {
        trace!("startup network");

        // initialize interfaces
        let mut interfaces = NetworkInterfaces::new();
        interfaces.refresh().await?;
        self.inner.lock().interfaces = interfaces;

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

        // release caches of available listener ports
        // this releases the 'first bound' ports we use to guarantee
        // that we have ports available to us
        self.free_bound_first_ports();

        info!("network started");
        self.inner.lock().network_started = true;
        Ok(())
    }

    pub fn needs_restart(&self) -> bool {
        self.inner.lock().network_needs_restart
    }

    pub async fn shutdown(&self) {
        info!("stopping network");

        let network_manager = self.network_manager();
        let routing_table = self.routing_table();

        // Reset state

        // Drop all dial info
        routing_table.clear_dial_info_details();

        // Cancels all async background tasks by dropping join handles
        *self.inner.lock() = Self::new_inner(network_manager);

        info!("network stopped");
    }

    //////////////////////////////////////////
    pub fn get_network_class(&self) -> Option<NetworkClass> {
        let inner = self.inner.lock();
        if !inner.network_started {
            return None;
        }

        // If we've fixed the network class, return it rather than calculating it
        if inner.network_class.is_some() {
            return inner.network_class;
        }

        // Go through our global dialinfo and see what our best network class is
        let mut network_class = NetworkClass::Invalid;
        for did in inner.routing_table.global_dial_info_details() {
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
                inner.routing_table.clone(),
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

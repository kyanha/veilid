pub use crate::rpc_processor::InfoAnswer;
use crate::*;
use attachment_manager::AttachmentManager;
use network_manager::NetworkManager;
use rpc_processor::{RPCError, RPCProcessor};
use xx::*;

pub use crate::dht::key::{generate_secret, DHTKey, DHTKeySecret};
pub use crate::xx::{
    IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, SystemPinBoxFuture,
    ToSocketAddrs,
};
pub use alloc::string::ToString;
pub use core::str::FromStr;

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default, PartialOrd, PartialEq, Eq, Ord)]
pub struct NodeId {
    pub key: DHTKey,
}
impl NodeId {
    pub fn new(key: DHTKey) -> Self {
        Self { key: key }
    }
}

#[derive(Clone, Debug, Default, PartialOrd, PartialEq, Eq, Ord)]
pub struct ValueKey {
    pub key: DHTKey,
    pub subkey: Option<String>,
}
impl ValueKey {
    pub fn new(key: DHTKey) -> Self {
        Self {
            key: key,
            subkey: None,
        }
    }
    pub fn new_subkey(key: DHTKey, subkey: String) -> Self {
        Self {
            key: key,
            subkey: if subkey.len() == 0 {
                None
            } else {
                Some(subkey)
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialOrd, PartialEq, Eq, Ord)]
pub struct BlockId {
    pub key: DHTKey,
}
impl BlockId {
    pub fn new(key: DHTKey) -> Self {
        Self { key: key }
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct SenderInfo {
    pub socket_address: Option<SocketAddr>,
}

impl Default for SenderInfo {
    fn default() -> Self {
        Self {
            socket_address: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NodeInfo {
    pub can_route: bool,
    pub will_route: bool,
    pub can_tunnel: bool,
    pub will_tunnel: bool,
    pub can_signal_lease: bool,
    pub will_signal_lease: bool,
    pub can_relay_lease: bool,
    pub will_relay_lease: bool,
    pub can_validate_dial_info: bool,
    pub will_validate_dial_info: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub enum ProtocolType {
    UDP,
    TCP,
    WS,
    WSS,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub enum ProtocolAddressType {
    UDPv4,
    UDPv6,
    TCPv4,
    TCPv6,
    WS,
    WSS,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub enum Address {
    IPV4(Ipv4Addr),
    IPV6(Ipv6Addr),
    Hostname(String),
}

impl Address {
    pub fn from_socket_addr(sa: SocketAddr) -> Address {
        match sa {
            SocketAddr::V4(v4) => Address::IPV4(*v4.ip()),
            SocketAddr::V6(v6) => Address::IPV6(*v6.ip()),
        }
    }
    pub fn address_string(&self) -> String {
        match self {
            Address::IPV4(v4) => v4.to_string(),
            Address::IPV6(v6) => v6.to_string(),
            Address::Hostname(h) => h.clone(),
        }
    }
    pub fn address_string_with_port(&self, port: u16) -> String {
        match self {
            Address::IPV4(v4) => format!("{}:{}", v4.to_string(), port),
            Address::IPV6(v6) => format!("[{}]:{}", v6.to_string(), port),
            Address::Hostname(h) => format!("{}:{}", h.clone(), port),
        }
    }
    pub fn resolve(&self) -> Result<IpAddr, ()> {
        match self {
            Self::IPV4(a) => Ok(IpAddr::V4(a.clone())),
            Self::IPV6(a) => Ok(IpAddr::V6(a.clone())),
            Self::Hostname(h) => h.parse().map_err(drop),
        }
    }
    pub fn address(&self) -> Result<IpAddr, ()> {
        match self {
            Self::IPV4(a) => Ok(IpAddr::V4(a.clone())),
            Self::IPV6(a) => Ok(IpAddr::V6(a.clone())),
            Self::Hostname(_) => Err(()),
        }
    }
    pub fn to_socket_addr(&self, port: u16) -> Result<SocketAddr, ()> {
        let addr = self.address()?;
        Ok(SocketAddr::new(addr, port))
    }
}

impl core::str::FromStr for Address {
    type Err = ();
    fn from_str(host: &str) -> Result<Address, ()> {
        if let Some(addr) = Ipv4Addr::from_str(host).ok() {
            Ok(Address::IPV4(addr))
        } else if let Some(addr) = Ipv6Addr::from_str(host).ok() {
            Ok(Address::IPV6(addr))
        } else if !host.is_empty() {
            Ok(Address::Hostname(host.to_string()))
        } else {
            Err(())
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct DialInfoUDP {
    pub address: Address,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct DialInfoTCP {
    pub address: Address,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct DialInfoWS {
    pub fqdn: String,
    pub port: u16,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub struct DialInfoWSS {
    pub fqdn: String,
    pub port: u16,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub enum DialInfo {
    UDP(DialInfoUDP),
    TCP(DialInfoTCP),
    WS(DialInfoWS),
    WSS(DialInfoWSS),
}

impl DialInfo {
    pub fn udp_from_socketaddr(socketaddr: SocketAddr) -> Self {
        Self::UDP(DialInfoUDP {
            address: Address::from_socket_addr(socketaddr),
            port: socketaddr.port(),
        })
    }
    pub fn tcp_from_socketaddr(socketaddr: SocketAddr) -> Self {
        Self::TCP(DialInfoTCP {
            address: Address::from_socket_addr(socketaddr),
            port: socketaddr.port(),
        })
    }
    pub fn udp(address: Address, port: u16) -> Self {
        if let Address::Hostname(_) = address {
            panic!("invalid address type for protocol")
        }
        Self::UDP(DialInfoUDP {
            address: address,
            port: port,
        })
    }
    pub fn tcp(address: Address, port: u16) -> Self {
        if let Address::Hostname(_) = address {
            panic!("invalid address type for protocol")
        }
        Self::TCP(DialInfoTCP {
            address: address,
            port: port,
        })
    }
    pub fn ws(fqdn: String, port: u16, path: String) -> Self {
        Self::WS(DialInfoWS {
            fqdn: fqdn,
            port: port,
            path: path,
        })
    }
    pub fn wss(fqdn: String, port: u16, path: String) -> Self {
        Self::WSS(DialInfoWSS {
            fqdn: fqdn,
            port: port,
            path: path,
        })
    }
    pub fn protocol_type(&self) -> ProtocolType {
        match self {
            Self::UDP(_) => ProtocolType::UDP,
            Self::TCP(_) => ProtocolType::TCP,
            Self::WS(_) => ProtocolType::WS,
            Self::WSS(_) => ProtocolType::WSS,
        }
    }

    pub fn protocol_address_type(&self) -> ProtocolAddressType {
        match self {
            Self::UDP(di) => match di.address {
                Address::IPV4(_) => ProtocolAddressType::UDPv4,
                Address::IPV6(_) => ProtocolAddressType::UDPv6,
                Address::Hostname(_) => panic!("invalid address type for protocol"),
            },
            Self::TCP(di) => match di.address {
                Address::IPV4(_) => ProtocolAddressType::TCPv4,
                Address::IPV6(_) => ProtocolAddressType::TCPv6,
                Address::Hostname(_) => panic!("invalid address type for protocol"),
            },
            Self::WS(_) => ProtocolAddressType::WS,
            Self::WSS(_) => ProtocolAddressType::WSS,
        }
    }

    pub fn try_udp_v4(&self) -> Option<SocketAddrV4> {
        match self {
            Self::UDP(v) => match v.address.to_socket_addr(v.port).ok() {
                Some(x) => match x {
                    SocketAddr::V4(v4) => Some(v4),
                    _ => None,
                },
                None => None,
            },
            _ => None,
        }
    }

    pub fn try_udp_v6(&self) -> Option<SocketAddrV6> {
        match self {
            Self::UDP(v) => match v.address.to_socket_addr(v.port).ok() {
                Some(x) => match x {
                    SocketAddr::V6(v6) => Some(v6),
                    _ => None,
                },
                None => None,
            },
            _ => None,
        }
    }

    pub fn try_tcp_v4(&self) -> Option<SocketAddrV4> {
        match self {
            Self::TCP(v) => match v.address.to_socket_addr(v.port).ok() {
                Some(x) => match x {
                    SocketAddr::V4(v4) => Some(v4),
                    _ => None,
                },
                None => None,
            },
            _ => None,
        }
    }

    pub fn try_tcp_v6(&self) -> Option<SocketAddrV6> {
        match self {
            Self::TCP(v) => match v.address.to_socket_addr(v.port).ok() {
                Some(x) => match x {
                    SocketAddr::V6(v6) => Some(v6),
                    _ => None,
                },
                None => None,
            },
            _ => None,
        }
    }

    pub fn try_ws(&self) -> Option<String> {
        match self {
            Self::WS(v) => Some(v.fqdn.clone()),
            _ => None,
        }
    }

    pub fn try_wss(&self) -> Option<String> {
        match self {
            Self::WSS(v) => Some(v.fqdn.clone()),
            _ => None,
        }
    }

    pub fn address_string(&self) -> String {
        match self {
            Self::UDP(di) => di.address.address_string(),
            Self::TCP(di) => di.address.address_string(),
            Self::WS(di) => di.fqdn.clone(),
            Self::WSS(di) => di.fqdn.clone(),
        }
    }
    pub fn address_string_with_port(&self) -> String {
        match self {
            Self::UDP(di) => di.address.address_string_with_port(di.port),
            Self::TCP(di) => di.address.address_string_with_port(di.port),
            Self::WS(di) => format!("{}:{}", di.fqdn.clone(), di.port),
            Self::WSS(di) => format!("{}:{}", di.fqdn.clone(), di.port),
        }
    }
    pub fn all_but_path(&self) -> String {
        match self {
            Self::UDP(di) => format!("udp://{}", di.address.address_string_with_port(di.port)),
            Self::TCP(di) => format!("tcp://{}", di.address.address_string_with_port(di.port)),
            Self::WS(di) => format!("ws://{}:{}", di.fqdn.clone(), di.port),
            Self::WSS(di) => format!("wss://{}:{}", di.fqdn.clone(), di.port),
        }
    }

    pub fn to_url_string(&self, user: Option<String>) -> String {
        let user_string = match user {
            Some(u) => format!("{}@", u),
            None => "".to_owned(),
        };
        match self {
            Self::UDP(di) => format!(
                "udp://{}{}",
                user_string,
                di.address.address_string_with_port(di.port)
            ),
            Self::TCP(di) => format!(
                "tcp://{}{}",
                user_string,
                di.address.address_string_with_port(di.port)
            ),
            Self::WS(di) => format!(
                "ws://{}{}:{}{}",
                user_string,
                di.fqdn.clone(),
                di.port,
                prepend_slash(di.path.clone())
            ),
            Self::WSS(di) => format!(
                "wss://{}{}:{}{}",
                user_string,
                di.fqdn.clone(),
                di.port,
                prepend_slash(di.path.clone())
            ),
        }
    }

    pub fn resolve(&self) -> Result<IpAddr, ()> {
        match self {
            Self::UDP(di) => {
                let addr = di.address.resolve()?;
                return Ok(addr);
            }
            Self::TCP(di) => {
                let addr = di.address.resolve()?;
                return Ok(addr);
            }
            Self::WS(di) => {
                let addr: IpAddr = di.fqdn.parse().map_err(drop)?;
                return Ok(addr);
            }
            Self::WSS(di) => {
                let addr: IpAddr = di.fqdn.parse().map_err(drop)?;
                return Ok(addr);
            }
        }
    }
    pub fn address(&self) -> Result<IpAddr, ()> {
        match self {
            Self::UDP(di) => di.address.address(),
            Self::TCP(di) => di.address.address(),
            Self::WS(_) => Err(()),
            Self::WSS(_) => Err(()),
        }
    }
    pub fn port(&self) -> u16 {
        match self {
            Self::UDP(di) => di.port,
            Self::TCP(di) => di.port,
            Self::WS(di) => di.port,
            Self::WSS(di) => di.port,
        }
    }
    pub fn path(&self) -> Result<String, ()> {
        match self {
            Self::UDP(_) => Err(()),
            Self::TCP(_) => Err(()),
            Self::WS(di) => Ok(di.path.clone()),
            Self::WSS(di) => Ok(di.path.clone()),
        }
    }
    pub fn to_socket_addr(&self) -> Result<SocketAddr, ()> {
        match self {
            Self::UDP(di) => Ok(SocketAddr::new(di.address.address()?, di.port)),
            Self::TCP(di) => Ok(SocketAddr::new(di.address.address()?, di.port)),
            Self::WS(_) => Err(()),
            Self::WSS(_) => Err(()),
        }
    }

    pub fn is_public(&self) -> Result<bool, String> {
        let addr = self
            .resolve()
            .map_err(|_| "failed to resolve address".to_owned())?;
        Ok(ipaddr_is_global(&addr))
    }

    pub fn is_private(&self) -> Result<bool, String> {
        let addr = self
            .resolve()
            .map_err(|_| "failed to resolve address".to_owned())?;
        Ok(match addr {
            IpAddr::V4(a) => ipv4addr_is_private(&a),
            IpAddr::V6(a) => ipv6addr_is_unicast_site_local(&a),
        })
    }
    pub fn is_valid(&self) -> Result<bool, String> {
        Ok(self.is_public()? || self.is_private()?)
    }
    pub fn is_loopback(&self) -> Result<bool, String> {
        let addr = self
            .resolve()
            .map_err(|_| "failed to resolve address".to_owned())?;
        Ok(ipaddr_is_loopback(&addr))
    }
}

impl ToString for DialInfo {
    fn to_string(&self) -> String {
        self.to_url_string(None)
    }
}

impl Default for DialInfo {
    fn default() -> Self {
        Self::UDP(DialInfoUDP {
            address: Address::IPV4(Ipv4Addr::new(0, 0, 0, 0)),
            port: 0u16,
        })
    }
}

//////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug)]
pub enum PeerScope {
    All,
    Public,
    Private,
}

#[derive(Clone, Debug, Default)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub dial_infos: Vec<DialInfo>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct PeerAddress {
    pub address: Address,
    pub port: u16,
    pub protocol_type: ProtocolType,
}

impl PeerAddress {
    pub fn new(address: Address, port: u16, protocol_type: ProtocolType) -> Self {
        Self {
            address: address,
            port: port,
            protocol_type: protocol_type,
        }
    }

    pub fn to_socket_addr(&self) -> Result<SocketAddr, ()> {
        self.address.to_socket_addr(self.port)
    }
    pub fn protocol_address_type(&self) -> ProtocolAddressType {
        match self.protocol_type {
            ProtocolType::UDP => match self.address {
                Address::IPV4(_) => ProtocolAddressType::UDPv4,
                Address::IPV6(_) => ProtocolAddressType::UDPv6,
                Address::Hostname(_) => panic!("invalid address type for protocol"),
            },
            ProtocolType::TCP => match self.address {
                Address::IPV4(_) => ProtocolAddressType::TCPv4,
                Address::IPV6(_) => ProtocolAddressType::TCPv6,
                Address::Hostname(_) => panic!("invalid address type for protocol"),
            },
            ProtocolType::WS => ProtocolAddressType::WS,
            ProtocolType::WSS => ProtocolAddressType::WSS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionDescriptor {
    pub remote: PeerAddress,
    pub local: Option<SocketAddr>,
}

impl ConnectionDescriptor {
    pub fn new(remote: PeerAddress, local: SocketAddr) -> Self {
        Self {
            remote: remote,
            local: Some(local),
        }
    }
    pub fn new_no_local(remote: PeerAddress) -> Self {
        Self {
            remote: remote,
            local: None,
        }
    }
    pub fn protocol_type(&self) -> ProtocolType {
        self.remote.protocol_type
    }
    pub fn protocol_address_type(&self) -> ProtocolAddressType {
        self.remote.protocol_address_type()
    }
}

//////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct NodeDialInfoSingle {
    pub node_id: NodeId,
    pub dial_info: DialInfo,
}

impl core::str::FromStr for NodeDialInfoSingle {
    type Err = ();
    fn from_str(url: &str) -> Result<NodeDialInfoSingle, ()> {
        let mut cur_url = url;
        let proto;
        if url.starts_with("udp://") {
            cur_url = &cur_url[6..];
            proto = ProtocolType::UDP;
        } else if url.starts_with("tcp://") {
            cur_url = &cur_url[6..];
            proto = ProtocolType::TCP;
        } else if url.starts_with("ws://") {
            cur_url = &cur_url[5..];
            proto = ProtocolType::WS;
        } else if url.starts_with("wss://") {
            cur_url = &cur_url[6..];
            proto = ProtocolType::WSS;
        } else {
            return Err(());
        }

        // parse out node id if we have one
        let node_id = match cur_url.find('@') {
            Some(x) => {
                let n = NodeId::new(DHTKey::try_decode(&cur_url[0..x]).map_err(drop)?);
                cur_url = &cur_url[x + 1..];
                n
            }
            None => {
                return Err(());
            }
        };

        // parse out address
        let address = match cur_url.rfind(':') {
            Some(x) => {
                let mut h = &cur_url[0..x];
                cur_url = &cur_url[x + 1..];

                match proto {
                    ProtocolType::WS | ProtocolType::WSS => Address::Hostname(h.to_string()),
                    _ => {
                        // peel off square brackets on ipv6 address
                        if x >= 2 && &h[0..1] == "[" && &h[(h.len() - 1)..] == "]" {
                            h = &h[1..(h.len() - 1)];
                        }
                        Address::from_str(h)?
                    }
                }
            }
            None => {
                return Err(());
            }
        };

        // parse out port
        let pathstart = cur_url.find('/').unwrap_or(cur_url.len());
        let port = u16::from_str(&cur_url[0..pathstart]).map_err(drop)?;
        cur_url = &cur_url[pathstart..];

        // build NodeDialInfoSingle
        Ok(NodeDialInfoSingle {
            node_id: node_id,
            dial_info: match proto {
                ProtocolType::UDP => DialInfo::udp(address, port),
                ProtocolType::TCP => DialInfo::tcp(address, port),
                ProtocolType::WS => {
                    DialInfo::ws(address.address_string(), port, cur_url.to_string())
                }
                ProtocolType::WSS => {
                    DialInfo::wss(address.address_string(), port, cur_url.to_string())
                }
            },
        })
    }
}

impl ToString for NodeDialInfoSingle {
    fn to_string(&self) -> String {
        self.dial_info
            .to_url_string(Some(self.node_id.key.encode()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct LatencyStats {
    pub fastest: u64, // fastest latency in the ROLLING_LATENCIES_SIZE last latencies
    pub average: u64, // average latency over the ROLLING_LATENCIES_SIZE last latencies
    pub slowest: u64, // slowest latency in the ROLLING_LATENCIES_SIZE last latencies
}

#[derive(Clone, Debug, Default)]
pub struct TransferStatsDownUp {
    pub down: TransferStats,
    pub up: TransferStats,
}

#[derive(Clone, Debug, Default)]
pub struct TransferStats {
    pub total: u64,   // total amount transferred ever
    pub maximum: u64, // maximum rate over the ROLLING_TRANSFERS_SIZE last amounts
    pub average: u64, // average rate over the ROLLING_TRANSFERS_SIZE last amounts
    pub minimum: u64, // minimum rate over the ROLLING_TRANSFERS_SIZE last amounts
}

#[derive(Clone, Debug, Default)]
pub struct PingStats {
    pub in_flight: u32,         // number of pings issued that have yet to be answered
    pub total_sent: u32,        // number of pings that have been sent in the total_time range
    pub total_returned: u32, // number of pings that have been returned by the node in the total_time range
    pub consecutive_pongs: u32, // number of pongs that have been received and returned consecutively without a lost ping
    pub last_pinged: Option<u64>, // when the peer was last pinged
    pub first_consecutive_pong_time: Option<u64>, // the timestamp of the first pong in a series of consecutive pongs
    pub recent_lost_pings: u32, // number of pings that have been lost since we lost reliability
}

#[derive(Clone, Debug, Default)]
pub struct PeerStats {
    pub time_added: u64,               // when the peer was added to the routing table
    pub last_seen: Option<u64>,        // when the peer was last seen for any reason
    pub ping_stats: PingStats,         // information about pings
    pub latency: Option<LatencyStats>, // latencies for communications with the peer
    pub transfer: TransferStatsDownUp, // Stats for communications with the peer
    pub node_info: Option<NodeInfo>,   // Last known node info
}

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub type ValueChangeCallback =
            Arc<dyn Fn(ValueKey, Vec<u8>) -> SystemPinBoxFuture<()> + 'static>;
    } else {
        pub type ValueChangeCallback =
            Arc<dyn Fn(ValueKey, Vec<u8>) -> SystemPinBoxFuture<()> + Send + Sync + 'static>;
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Ord)]
pub enum VeilidAPIError {
    Timeout,
    Shutdown,
    NodeNotFound(NodeId),
    NoDialInfo(NodeId),
    Internal(String),
    Unimplemented(String),
}

fn convert_rpc_error(x: RPCError) -> VeilidAPIError {
    match x {
        RPCError::Timeout => VeilidAPIError::Timeout,
        RPCError::Unimplemented(s) => VeilidAPIError::Unimplemented(s),
        RPCError::Internal(s) => VeilidAPIError::Internal(s),
        RPCError::Protocol(s) => VeilidAPIError::Internal(s),
        RPCError::InvalidFormat => VeilidAPIError::Internal("Invalid packet format".to_owned()),
    }
}

macro_rules! map_rpc_error {
    () => {
        |x| convert_rpc_error(x)
    };
}

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Ord)]
pub enum TunnelMode {
    Raw,
    Turn,
}

type TunnelId = u64;

#[derive(Clone, Debug)]
pub struct TunnelEndpoint {
    pub node_id: NodeId,          // the node id of the tunnel endpoint
    pub dial_info: Vec<DialInfo>, // multiple ways of how to get to the node
    pub mode: TunnelMode,
}

impl Default for TunnelEndpoint {
    fn default() -> Self {
        Self {
            node_id: NodeId::default(),
            dial_info: Vec::new(),
            mode: TunnelMode::Raw,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FullTunnel {
    pub id: TunnelId,
    pub timeout: u64,
    pub local: TunnelEndpoint,
    pub remote: TunnelEndpoint,
}

#[derive(Clone, Debug, Default)]
pub struct PartialTunnel {
    pub id: TunnelId,
    pub timeout: u64,
    pub local: TunnelEndpoint,
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct RouteHopSpec {
    pub dial_info: NodeDialInfoSingle,
}

#[derive(Clone, Debug, Default)]
pub struct PrivateRouteSpec {
    //
    pub public_key: DHTKey,
    pub secret_key: DHTKeySecret,
    pub hops: Vec<RouteHopSpec>,
}

#[derive(Clone, Debug, Default)]
pub struct SafetyRouteSpec {
    pub public_key: DHTKey,
    pub secret_key: DHTKeySecret,
    pub hops: Vec<RouteHopSpec>,
}

impl SafetyRouteSpec {
    pub fn new() -> Self {
        let (pk, sk) = generate_secret();
        SafetyRouteSpec {
            public_key: pk,
            secret_key: sk,
            hops: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RoutingContextOptions {
    pub safety_route_spec: Option<SafetyRouteSpec>,
    pub private_route_spec: Option<PrivateRouteSpec>,
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct SearchDHTAnswer {
    pub node_id: NodeId,
    pub dial_info: Vec<DialInfo>,
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct RoutingContextInner {
    api: VeilidAPI,
    options: RoutingContextOptions,
}

impl Drop for RoutingContextInner {
    fn drop(&mut self) {
        // self.api
        //     .borrow_mut()
        //     .routing_contexts
        //     //.remove(&self.id);
    }
}

#[derive(Clone)]
pub struct RoutingContext {
    inner: Arc<Mutex<RoutingContextInner>>,
}

impl RoutingContext {
    fn new(api: VeilidAPI, options: RoutingContextOptions) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RoutingContextInner {
                api: api,
                options: options,
            })),
        }
    }

    pub fn api(&self) -> VeilidAPI {
        self.inner.lock().api.clone()
    }

    ///////////////////////////////////
    ///

    pub async fn get_value(&self, _value_key: ValueKey) -> Result<Vec<u8>, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn set_value(
        &self,
        _value_key: ValueKey,
        _value: Vec<u8>,
    ) -> Result<bool, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn watch_value(
        &self,
        _value_key: ValueKey,
        _callback: ValueChangeCallback,
    ) -> Result<bool, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn cancel_watch_value(&self, _value_key: ValueKey) -> Result<bool, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn find_block(&self, _block_id: BlockId) -> Result<Vec<u8>, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn supply_block(&self, _block_id: BlockId) -> Result<bool, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn signal(&self, _data: Vec<u8>) -> Result<bool, VeilidAPIError> {
        panic!("unimplemented");
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

struct VeilidAPIInner {
    config: VeilidConfig,
    attachment_manager: AttachmentManager,
    core: VeilidCore,
    network_manager: NetworkManager,
    is_shutdown: bool,
}

impl Drop for VeilidAPIInner {
    fn drop(&mut self) {
        if !self.is_shutdown {
            intf::spawn_local(self.core.clone().internal_shutdown()).detach();
        }
    }
}

#[derive(Clone)]
pub struct VeilidAPI {
    inner: Arc<Mutex<VeilidAPIInner>>,
}

impl VeilidAPI {
    pub fn new(attachment_manager: AttachmentManager, core: VeilidCore) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VeilidAPIInner {
                config: attachment_manager.config(),
                attachment_manager: attachment_manager.clone(),
                core: core,
                network_manager: attachment_manager.network_manager(),
                is_shutdown: false,
            })),
        }
    }

    pub fn config(&self) -> VeilidConfig {
        self.inner.lock().config.clone()
    }

    fn attachment_manager(&self) -> AttachmentManager {
        self.inner.lock().attachment_manager.clone()
    }

    fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }

    fn rpc_processor(&self) -> RPCProcessor {
        self.inner.lock().network_manager.rpc_processor()
    }

    pub async fn shutdown(&self) {
        let mut inner = self.inner.lock();
        if !inner.is_shutdown {
            inner.core.clone().internal_shutdown().await;
            inner.is_shutdown = true;
        }
    }
    pub fn is_shutdown(&self) -> bool {
        self.inner.lock().is_shutdown
    }

    fn verify_not_shutdown(&self) -> Result<(), VeilidAPIError> {
        if self.is_shutdown() {
            return Err(VeilidAPIError::Shutdown);
        }
        Ok(())
    }

    ////////////////////////////////////////////////////////////////
    // Attach/Detach

    // issue state changed updates for updating clients
    pub async fn send_state_update(&self) {
        trace!("VeilidCore::send_state_update");
        let attachment_manager = self.attachment_manager().clone();
        attachment_manager.send_state_update().await;
    }

    // connect to the network
    pub async fn attach(&self) {
        trace!("VeilidCore::attach");
        let attachment_manager = self.attachment_manager().clone();
        attachment_manager.request_attach().await;
    }

    // disconnect from the network
    pub async fn detach(&self) {
        trace!("VeilidCore::detach");
        let attachment_manager = self.attachment_manager().clone();
        attachment_manager.request_detach().await;
    }

    // wait for state change
    // xxx: this should not use 'sleep', perhaps this function should be eliminated anyway
    pub async fn wait_for_state(&self, state: VeilidState) {
        loop {
            intf::sleep(500).await;
            match state {
                VeilidState::Attachment(cs) => {
                    if self.attachment_manager().get_state() == cs {
                        break;
                    }
                }
            }
        }
    }

    ////////////////////////////////////////////////////////////////
    // Direct Node Access (pretty much for testing only)

    pub async fn info(&self, node_id: NodeId) -> Result<InfoAnswer, VeilidAPIError> {
        self.verify_not_shutdown()?;

        let rpc = self.rpc_processor();
        let routing_table = rpc.routing_table();
        let node_ref = match routing_table.lookup_node_ref(node_id.key) {
            None => return Err(VeilidAPIError::NodeNotFound(node_id)),
            Some(nr) => nr,
        };
        let info_answer = rpc
            .rpc_call_info(node_ref)
            .await
            .map_err(map_rpc_error!())?;
        Ok(info_answer)
    }

    pub async fn validate_dial_info(
        &self,
        node_id: NodeId,
        dial_info: DialInfo,
        redirect: bool,
        alternate_port: bool,
    ) -> Result<bool, VeilidAPIError> {
        self.verify_not_shutdown()?;

        let rpc = self.rpc_processor();
        let routing_table = rpc.routing_table();
        let node_ref = match routing_table.lookup_node_ref(node_id.key) {
            None => return Err(VeilidAPIError::NodeNotFound(node_id)),
            Some(nr) => nr,
        };
        rpc.rpc_call_validate_dial_info(node_ref.clone(), dial_info, redirect, alternate_port)
            .await
            .map_err(map_rpc_error!())
    }

    pub async fn search_dht(&self, node_id: NodeId) -> Result<SearchDHTAnswer, VeilidAPIError> {
        self.verify_not_shutdown()?;
        let rpc_processor = self.rpc_processor();
        let config = self.config();
        let (count, fanout, timeout) = {
            let c = config.get();
            (
                c.network.dht.resolve_node_count,
                c.network.dht.resolve_node_fanout,
                c.network.dht.resolve_node_timeout,
            )
        };

        let node_ref = rpc_processor
            .search_dht_single_key(node_id.key, count, fanout, timeout)
            .await
            .map_err(map_rpc_error!())?;

        let answer = node_ref.operate(|e| SearchDHTAnswer {
            node_id: NodeId::new(node_ref.node_id()),
            dial_info: e.dial_info(),
        });

        Ok(answer)
    }

    pub async fn search_dht_multi(
        &self,
        node_id: NodeId,
    ) -> Result<Vec<SearchDHTAnswer>, VeilidAPIError> {
        self.verify_not_shutdown()?;

        let rpc_processor = self.rpc_processor();
        let config = self.config();
        let (count, fanout, timeout) = {
            let c = config.get();
            (
                c.network.dht.resolve_node_count,
                c.network.dht.resolve_node_fanout,
                c.network.dht.resolve_node_timeout,
            )
        };

        let node_refs = rpc_processor
            .search_dht_multi_key(node_id.key, count, fanout, timeout)
            .await
            .map_err(map_rpc_error!())?;

        let mut answer = Vec::<SearchDHTAnswer>::new();
        for nr in node_refs {
            let a = nr.operate(|e| SearchDHTAnswer {
                node_id: NodeId::new(nr.node_id()),
                dial_info: e.dial_info(),
            });
            answer.push(a);
        }

        Ok(answer)
    }

    ////////////////////////////////////////////////////////////////
    // Safety / Private Route Handling

    pub async fn new_safety_route_spec(
        &self,
        _hops: u8,
    ) -> Result<SafetyRouteSpec, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn new_private_route_spec(
        &self,
        _hops: u8,
    ) -> Result<PrivateRouteSpec, VeilidAPIError> {
        panic!("unimplemented");
    }

    ////////////////////////////////////////////////////////////////
    // Routing Contexts
    //
    // Safety route specified here is for _this_ node's anonymity as a sender, used via the 'route' operation
    // Private route specified here is for _this_ node's anonymity as a receiver, passed out via the 'respond_to' field for replies

    pub async fn safe_private(
        &self,
        safety_route_spec: SafetyRouteSpec,
        private_route_spec: PrivateRouteSpec,
    ) -> RoutingContext {
        self.routing_context(RoutingContextOptions {
            safety_route_spec: Some(safety_route_spec),
            private_route_spec: Some(private_route_spec),
        })
        .await
    }

    pub async fn safe_public(&self, safety_route_spec: SafetyRouteSpec) -> RoutingContext {
        self.routing_context(RoutingContextOptions {
            safety_route_spec: Some(safety_route_spec),
            private_route_spec: None,
        })
        .await
    }

    pub async fn unsafe_private(&self, private_route_spec: PrivateRouteSpec) -> RoutingContext {
        self.routing_context(RoutingContextOptions {
            safety_route_spec: None,
            private_route_spec: Some(private_route_spec),
        })
        .await
    }

    pub async fn unsafe_public(&self) -> RoutingContext {
        self.routing_context(RoutingContextOptions {
            safety_route_spec: None,
            private_route_spec: None,
        })
        .await
    }
    pub async fn routing_context(&self, options: RoutingContextOptions) -> RoutingContext {
        RoutingContext::new(self.clone(), options)
    }

    ////////////////////////////////////////////////////////////////
    // Tunnel Building

    pub async fn start_tunnel(
        &self,
        _endpoint_mode: TunnelMode,
        _depth: u8,
    ) -> Result<PartialTunnel, VeilidAPIError> {
        self.verify_not_shutdown()?;
        panic!("unimplemented");
    }

    pub async fn complete_tunnel(
        &self,
        _endpoint_mode: TunnelMode,
        _depth: u8,
        _partial_tunnel: PartialTunnel,
    ) -> Result<FullTunnel, VeilidAPIError> {
        self.verify_not_shutdown()?;
        panic!("unimplemented");
    }

    pub async fn cancel_tunnel(&self, _tunnel_id: TunnelId) -> Result<bool, VeilidAPIError> {
        self.verify_not_shutdown()?;
        panic!("unimplemented");
    }
}

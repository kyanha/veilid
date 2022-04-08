#![allow(dead_code)]

mod debug;
pub use debug::*;

use crate::*;

pub use crate::xx::{
    IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, SystemPinBoxFuture,
    ToSocketAddrs,
};
pub use alloc::string::ToString;
pub use attachment_manager::AttachmentManager;
pub use core::str::FromStr;
pub use dht::crypto::Crypto;
pub use dht::key::{generate_secret, DHTKey, DHTKeySecret};
pub use intf::BlockStore;
pub use intf::ProtectedStore;
pub use intf::TableStore;
pub use network_manager::NetworkManager;
pub use routing_table::RoutingTable;
pub use rpc_processor::InfoAnswer;

use core::fmt;
use core_context::{api_shutdown, VeilidCoreContext};
use rpc_processor::{RPCError, RPCProcessor};
use serde::*;
use xx::*;

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Ord, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VeilidAPIError {
    NotInitialized,
    AlreadyInitialized,
    Timeout,
    Shutdown,
    NodeNotFound {
        node_id: NodeId,
    },
    NoDialInfo {
        node_id: NodeId,
    },
    Internal {
        message: String,
    },
    Unimplemented {
        message: String,
    },
    ParseError {
        message: String,
        value: String,
    },
    InvalidArgument {
        context: String,
        argument: String,
        value: String,
    },
    MissingArgument {
        context: String,
        argument: String,
    },
}

impl fmt::Display for VeilidAPIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            VeilidAPIError::NotInitialized => write!(f, "VeilidAPIError::NotInitialized"),
            VeilidAPIError::AlreadyInitialized => write!(f, "VeilidAPIError::AlreadyInitialized"),
            VeilidAPIError::Timeout => write!(f, "VeilidAPIError::Timeout"),
            VeilidAPIError::Shutdown => write!(f, "VeilidAPIError::Shutdown"),
            VeilidAPIError::NodeNotFound { node_id } => {
                write!(f, "VeilidAPIError::NodeNotFound({})", node_id)
            }
            VeilidAPIError::NoDialInfo { node_id } => {
                write!(f, "VeilidAPIError::NoDialInfo({})", node_id)
            }
            VeilidAPIError::Internal { message } => {
                write!(f, "VeilidAPIError::Internal({})", message)
            }
            VeilidAPIError::Unimplemented { message } => {
                write!(f, "VeilidAPIError::Unimplemented({})", message)
            }
            VeilidAPIError::ParseError { message, value } => {
                write!(f, "VeilidAPIError::ParseError({}: {})", message, value)
            }
            VeilidAPIError::InvalidArgument {
                context,
                argument,
                value,
            } => {
                write!(
                    f,
                    "VeilidAPIError::InvalidArgument({}: {} = {})",
                    context, argument, value
                )
            }
            VeilidAPIError::MissingArgument { context, argument } => {
                write!(
                    f,
                    "VeilidAPIError::MissingArgument({}: {})",
                    context, argument
                )
            }
        }
    }
}

fn convert_rpc_error(x: RPCError) -> VeilidAPIError {
    match x {
        RPCError::Timeout => VeilidAPIError::Timeout,
        RPCError::Unimplemented(s) => VeilidAPIError::Unimplemented { message: s },
        RPCError::Internal(s) => VeilidAPIError::Internal { message: s },
        RPCError::Protocol(s) => VeilidAPIError::Internal { message: s },
        RPCError::InvalidFormat => VeilidAPIError::Internal {
            message: "Invalid packet format".to_owned(),
        },
    }
}

macro_rules! map_rpc_error {
    () => {
        |x| convert_rpc_error(x)
    };
}

macro_rules! parse_error {
    ($msg:expr, $val:expr) => {
        VeilidAPIError::ParseError {
            message: $msg.to_string(),
            value: $val.to_string(),
        }
    };
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum VeilidLogLevel {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

impl VeilidLogLevel {
    pub fn from_log_level(level: log::Level) -> VeilidLogLevel {
        match level {
            Level::Error => VeilidLogLevel::Error,
            Level::Warn => VeilidLogLevel::Warn,
            Level::Info => VeilidLogLevel::Info,
            Level::Debug => VeilidLogLevel::Debug,
            Level::Trace => VeilidLogLevel::Trace,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VeilidUpdate {
    Log {
        log_level: VeilidLogLevel,
        message: String,
    },
    Attachment {
        state: AttachmentState,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeilidState {
    pub attachment: AttachmentState,
}

/////////////////////////////////////////////////////////////////////////////////////////////////////
///
#[derive(Clone, Debug, Default, PartialOrd, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub struct NodeId {
    pub key: DHTKey,
}
impl NodeId {
    pub fn new(key: DHTKey) -> Self {
        assert!(key.valid);
        Self { key }
    }
}
impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.key.encode())
    }
}

#[derive(Clone, Debug, Default, PartialOrd, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub struct ValueKey {
    pub key: DHTKey,
    pub subkey: Option<String>,
}
impl ValueKey {
    pub fn new(key: DHTKey) -> Self {
        Self { key, subkey: None }
    }
    pub fn new_subkey(key: DHTKey, subkey: String) -> Self {
        Self {
            key,
            subkey: if subkey.is_empty() {
                None
            } else {
                Some(subkey)
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialOrd, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub struct BlockId {
    pub key: DHTKey,
}
impl BlockId {
    pub fn new(key: DHTKey) -> Self {
        Self { key }
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SenderInfo {
    pub socket_address: Option<SocketAddress>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum NetworkClass {
    Server = 0,               // S = Device with public IP and no UDP firewall
    Mapped = 1,               // M = Device with portmap behind any NAT
    FullConeNAT = 2,          // F = Device without portmap behind full-cone NAT
    AddressRestrictedNAT = 3, // A = Device without portmap behind address-only restricted NAT
    PortRestrictedNAT = 4,    // P = Device without portmap behind address-and-port restricted NAT
    OutboundOnly = 5,         // O = Outbound only
    WebApp = 6,               // W = PWA
    Invalid = 7,              // I = Invalid network class, unreachable or can not send packets
}

impl NetworkClass {
    // Can the node receive inbound requests without a relay?
    pub fn inbound_capable(&self) -> bool {
        matches!(
            self,
            Self::Server
                | Self::Mapped
                | Self::FullConeNAT
                | Self::AddressRestrictedNAT
                | Self::PortRestrictedNAT
        )
    }

    // Should an outbound relay be kept available?
    pub fn outbound_wants_relay(&self) -> bool {
        matches!(self, Self::WebApp)
    }

    // Is a signal required to do an inbound hole-punch?
    pub fn inbound_requires_signal(&self) -> bool {
        matches!(self, Self::AddressRestrictedNAT | Self::PortRestrictedNAT)
    }

    // Is some relay required either for signal or inbound relay or outbound relay?
    pub fn needs_relay(&self) -> bool {
        matches!(
            self,
            Self::AddressRestrictedNAT
                | Self::PortRestrictedNAT
                | Self::OutboundOnly
                | Self::WebApp
        )
    }

    // Must keepalive be used to preserve the public dialinfo in use?
    // Keepalive can be to either a
    pub fn dialinfo_requires_keepalive(&self) -> bool {
        matches!(
            self,
            Self::FullConeNAT
                | Self::AddressRestrictedNAT
                | Self::PortRestrictedNAT
                | Self::OutboundOnly
                | Self::WebApp
        )
    }

    // Can this node assist with signalling? Yes but only if it doesn't require signalling, itself.
    pub fn can_signal(&self) -> bool {
        self.inbound_capable() && !self.inbound_requires_signal()
    }

    // Can this node relay be an inbound relay?
    pub fn can_inbound_relay(&self) -> bool {
        matches!(self, Self::Server | Self::Mapped | Self::FullConeNAT)
    }

    // Is this node capable of validating dial info
    pub fn can_validate_dial_info(&self) -> bool {
        matches!(self, Self::Server | Self::Mapped | Self::FullConeNAT)
    }
}

impl Default for NetworkClass {
    fn default() -> Self {
        Self::Invalid
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeStatus {
    pub will_route: bool,
    pub will_tunnel: bool,
    pub will_signal: bool,
    pub will_relay: bool,
    pub will_validate_dial_info: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NodeInfo {
    pub network_class: NetworkClass,
    pub dial_infos: Vec<DialInfo>,
    pub relay_dial_infos: Vec<DialInfo>,
}

impl NodeInfo {
    pub fn first_filtered<F>(&self, filter: F) -> NodeInfo
    where
        F: Fn(&DialInfo) -> bool,
    {
        let mut node_info = NodeInfo::default();
        node_info.network_class = self.network_class;

        for di in &self.dial_infos {
            if filter(di) {
                node_info.dial_infos.push(di.clone());
                break;
            }
        }
        for di in &self.relay_dial_infos {
            if filter(di) {
                node_info.relay_dial_infos.push(di.clone());
                break;
            }
        }
        node_info
    }
    pub fn all_filtered<F>(&self, filter: F) -> NodeInfo
    where
        F: Fn(&DialInfo) -> bool,
    {
        let mut node_info = NodeInfo::default();
        node_info.network_class = self.network_class;

        for di in &self.dial_infos {
            if filter(di) {
                node_info.dial_infos.push(di.clone());
            }
        }
        for di in &self.relay_dial_infos {
            if filter(di) {
                node_info.relay_dial_infos.push(di.clone());
            }
        }
        node_info
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
// The derived ordering here is the order of preference, lower is preferred for connections
// Must match DialInfo order
pub enum ProtocolType {
    UDP,
    TCP,
    WS,
    WSS,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
pub enum AddressType {
    IPV4,
    IPV6,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
pub enum Address {
    IPV4(Ipv4Addr),
    IPV6(Ipv6Addr),
}

impl Default for Address {
    fn default() -> Self {
        Address::IPV4(Ipv4Addr::new(0, 0, 0, 0))
    }
}

impl Address {
    pub fn from_socket_addr(sa: SocketAddr) -> Address {
        match sa {
            SocketAddr::V4(v4) => Address::IPV4(*v4.ip()),
            SocketAddr::V6(v6) => Address::IPV6(*v6.ip()),
        }
    }
    pub fn address_type(&self) -> AddressType {
        match self {
            Address::IPV4(_) => AddressType::IPV4,
            Address::IPV6(_) => AddressType::IPV6,
        }
    }
    pub fn address_string(&self) -> String {
        match self {
            Address::IPV4(v4) => v4.to_string(),
            Address::IPV6(v6) => v6.to_string(),
        }
    }
    pub fn address_string_with_port(&self, port: u16) -> String {
        match self {
            Address::IPV4(v4) => format!("{}:{}", v4, port),
            Address::IPV6(v6) => format!("[{}]:{}", v6, port),
        }
    }
    pub fn is_global(&self) -> bool {
        match self {
            Address::IPV4(v4) => ipv4addr_is_global(v4),
            Address::IPV6(v6) => ipv6addr_is_global(v6),
        }
    }
    pub fn is_local(&self) -> bool {
        match self {
            Address::IPV4(v4) => ipv4addr_is_private(v4),
            Address::IPV6(v6) => ipv6addr_is_unicast_site_local(v6),
        }
    }
    pub fn to_ip_addr(&self) -> IpAddr {
        match self {
            Self::IPV4(a) => IpAddr::V4(*a),
            Self::IPV6(a) => IpAddr::V6(*a),
        }
    }
    pub fn to_socket_addr(&self, port: u16) -> SocketAddr {
        SocketAddr::new(self.to_ip_addr(), port)
    }
    pub fn to_canonical(&self) -> Address {
        match self {
            Address::IPV4(v4) => Address::IPV4(*v4),
            Address::IPV6(v6) => match v6.to_ipv4() {
                Some(v4) => Address::IPV4(v4),
                None => Address::IPV6(*v6),
            },
        }
    }
}

impl FromStr for Address {
    type Err = VeilidAPIError;
    fn from_str(host: &str) -> Result<Address, VeilidAPIError> {
        if let Ok(addr) = Ipv4Addr::from_str(host) {
            Ok(Address::IPV4(addr))
        } else if let Ok(addr) = Ipv6Addr::from_str(host) {
            Ok(Address::IPV6(addr))
        } else {
            Err(parse_error!("Address::from_str failed", host))
        }
    }
}

#[derive(
    Copy, Default, Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize,
)]
pub struct SocketAddress {
    address: Address,
    port: u16,
}

impl SocketAddress {
    pub fn new(address: Address, port: u16) -> Self {
        Self { address, port }
    }
    pub fn from_socket_addr(sa: SocketAddr) -> SocketAddress {
        Self {
            address: Address::from_socket_addr(sa),
            port: sa.port(),
        }
    }
    pub fn address(&self) -> Address {
        self.address
    }
    pub fn address_type(&self) -> AddressType {
        self.address.address_type()
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn to_canonical(&self) -> SocketAddress {
        SocketAddress {
            address: self.address.to_canonical(),
            port: self.port,
        }
    }
    pub fn to_ip_addr(&self) -> IpAddr {
        self.address.to_ip_addr()
    }
    pub fn to_socket_addr(&self) -> SocketAddr {
        self.address.to_socket_addr(self.port)
    }
}

impl fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.to_socket_addr())
    }
}

impl FromStr for SocketAddress {
    type Err = VeilidAPIError;
    fn from_str(s: &str) -> Result<SocketAddress, VeilidAPIError> {
        let sa = SocketAddr::from_str(s)
            .map_err(|e| parse_error!("Failed to parse SocketAddress", e))?;
        Ok(SocketAddress::from_socket_addr(sa))
    }
}

//////////////////////////////////////////////////////////////////

#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DialInfoFilter {
    pub peer_scope: PeerScope,
    pub protocol_type: Option<ProtocolType>,
    pub address_type: Option<AddressType>,
}

impl DialInfoFilter {
    pub fn all() -> Self {
        Self {
            peer_scope: PeerScope::All,
            protocol_type: None,
            address_type: None,
        }
    }
    pub fn global() -> Self {
        Self {
            peer_scope: PeerScope::Global,
            protocol_type: None,
            address_type: None,
        }
    }
    pub fn local() -> Self {
        Self {
            peer_scope: PeerScope::Local,
            protocol_type: None,
            address_type: None,
        }
    }
    pub fn scoped(peer_scope: PeerScope) -> Self {
        Self {
            peer_scope,
            protocol_type: None,
            address_type: None,
        }
    }
    pub fn with_protocol_type(mut self, protocol_type: ProtocolType) -> Self {
        self.protocol_type = Some(protocol_type);
        self
    }
    pub fn with_address_type(mut self, address_type: AddressType) -> Self {
        self.address_type = Some(address_type);
        self
    }
    pub fn is_empty(&self) -> bool {
        self.peer_scope == PeerScope::All
            && self.protocol_type.is_none()
            && self.address_type.is_none()
    }
}

impl fmt::Debug for DialInfoFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let mut out = String::new();
        out += &format!("{:?}", self.peer_scope);
        if let Some(pt) = self.protocol_type {
            out += &format!("+{:?}", pt);
        }
        if let Some(at) = self.address_type {
            out += &format!("+{:?}", at);
        }
        write!(f, "[{}]", out)
    }
}

pub trait MatchesDialInfoFilter {
    fn matches_filter(&self, filter: &DialInfoFilter) -> bool;
}

#[derive(Clone, Default, Debug, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct DialInfoUDP {
    pub socket_address: SocketAddress,
}

#[derive(Clone, Default, Debug, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct DialInfoTCP {
    pub socket_address: SocketAddress,
}

#[derive(Clone, Default, Debug, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct DialInfoWS {
    pub socket_address: SocketAddress,
    pub request: String,
}

#[derive(Clone, Default, Debug, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct DialInfoWSS {
    pub socket_address: SocketAddress,
    pub request: String,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum DialInfoClass {
    Direct,
    Relay,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
// The derived ordering here is the order of preference, lower is preferred for connections
// Must match ProtocolType order
pub enum DialInfo {
    UDP(DialInfoUDP),
    TCP(DialInfoTCP),
    WS(DialInfoWS),
    WSS(DialInfoWSS),
}
impl Default for DialInfo {
    fn default() -> Self {
        DialInfo::UDP(DialInfoUDP::default())
    }
}

impl fmt::Display for DialInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            DialInfo::UDP(di) => write!(f, "udp|{}", di.socket_address),
            DialInfo::TCP(di) => write!(f, "tcp|{}", di.socket_address),
            DialInfo::WS(di) => write!(f, "ws|{}|{}", di.socket_address, di.request),
            DialInfo::WSS(di) => write!(f, "wss|{}|{}", di.socket_address, di.request),
        }
    }
}

impl FromStr for DialInfo {
    type Err = VeilidAPIError;
    fn from_str(s: &str) -> Result<DialInfo, VeilidAPIError> {
        let (proto, rest) = s
            .split_once('|')
            .ok_or_else(|| parse_error!("DialInfo::from_str missing protocol '|' separator", s))?;
        match proto {
            "udp" => {
                let socket_address = SocketAddress::from_str(rest)?;
                Ok(DialInfo::udp(socket_address))
            }
            "tcp" => {
                let socket_address = SocketAddress::from_str(rest)?;
                Ok(DialInfo::tcp(socket_address))
            }
            "ws" => {
                let (sa, rest) = rest.split_once('|').ok_or_else(|| {
                    parse_error!("DialInfo::from_str missing socket address '|' separator", s)
                })?;
                let socket_address = SocketAddress::from_str(sa)?;
                DialInfo::try_ws(socket_address, format!("ws://{}", rest))
            }
            "wss" => {
                let (sa, rest) = rest.split_once('|').ok_or_else(|| {
                    parse_error!("DialInfo::from_str missing socket address '|' separator", s)
                })?;
                let socket_address = SocketAddress::from_str(sa)?;
                DialInfo::try_wss(socket_address, format!("wss://{}", rest))
            }
            _ => Err(parse_error!("DialInfo::from_str has invalid scheme", s)),
        }
    }
}

impl DialInfo {
    pub fn udp_from_socketaddr(socket_addr: SocketAddr) -> Self {
        Self::UDP(DialInfoUDP {
            socket_address: SocketAddress::from_socket_addr(socket_addr).to_canonical(),
        })
    }
    pub fn tcp_from_socketaddr(socket_addr: SocketAddr) -> Self {
        Self::TCP(DialInfoTCP {
            socket_address: SocketAddress::from_socket_addr(socket_addr).to_canonical(),
        })
    }
    pub fn udp(socket_address: SocketAddress) -> Self {
        Self::UDP(DialInfoUDP {
            socket_address: socket_address.to_canonical(),
        })
    }
    pub fn tcp(socket_address: SocketAddress) -> Self {
        Self::TCP(DialInfoTCP {
            socket_address: socket_address.to_canonical(),
        })
    }
    pub fn try_ws(socket_address: SocketAddress, url: String) -> Result<Self, VeilidAPIError> {
        let split_url = SplitUrl::from_str(&url)
            .map_err(|e| parse_error!(format!("unable to split WS url: {}", e), url))?;
        if split_url.scheme != "ws" || !url.starts_with("ws://") {
            return Err(parse_error!("incorrect scheme for WS dialinfo", url));
        }
        let url_port = split_url.port.unwrap_or(80u16);
        if url_port != socket_address.port() {
            return Err(parse_error!(
                "socket address port doesn't match url port",
                url
            ));
        }
        Ok(Self::WS(DialInfoWS {
            socket_address: socket_address.to_canonical(),
            request: url[5..].to_string(),
        }))
    }
    pub fn try_wss(socket_address: SocketAddress, url: String) -> Result<Self, VeilidAPIError> {
        let split_url = SplitUrl::from_str(&url)
            .map_err(|e| parse_error!(format!("unable to split WSS url: {}", e), url))?;
        if split_url.scheme != "wss" || !url.starts_with("wss://") {
            return Err(parse_error!("incorrect scheme for WSS dialinfo", url));
        }
        let url_port = split_url.port.unwrap_or(443u16);
        if url_port != socket_address.port() {
            return Err(parse_error!(
                "socket address port doesn't match url port",
                url
            ));
        }
        if Address::from_str(&split_url.host).is_ok() {
            return Err(parse_error!(
                "WSS url can not use address format, only hostname format",
                url
            ));
        }
        Ok(Self::WSS(DialInfoWSS {
            socket_address: socket_address.to_canonical(),
            request: url[6..].to_string(),
        }))
    }
    pub fn protocol_type(&self) -> ProtocolType {
        match self {
            Self::UDP(_) => ProtocolType::UDP,
            Self::TCP(_) => ProtocolType::TCP,
            Self::WS(_) => ProtocolType::WS,
            Self::WSS(_) => ProtocolType::WSS,
        }
    }
    pub fn address_type(&self) -> AddressType {
        self.socket_address().address_type()
    }
    pub fn socket_address(&self) -> SocketAddress {
        match self {
            Self::UDP(di) => di.socket_address,
            Self::TCP(di) => di.socket_address,
            Self::WS(di) => di.socket_address,
            Self::WSS(di) => di.socket_address,
        }
    }
    pub fn to_ip_addr(&self) -> IpAddr {
        match self {
            Self::UDP(di) => di.socket_address.to_ip_addr(),
            Self::TCP(di) => di.socket_address.to_ip_addr(),
            Self::WS(di) => di.socket_address.to_ip_addr(),
            Self::WSS(di) => di.socket_address.to_ip_addr(),
        }
    }
    pub fn port(&self) -> u16 {
        match self {
            Self::UDP(di) => di.socket_address.port,
            Self::TCP(di) => di.socket_address.port,
            Self::WS(di) => di.socket_address.port,
            Self::WSS(di) => di.socket_address.port,
        }
    }
    pub fn set_port(&mut self, port: u16) {
        match self {
            Self::UDP(di) => di.socket_address.port = port,
            Self::TCP(di) => di.socket_address.port = port,
            Self::WS(di) => di.socket_address.port = port,
            Self::WSS(di) => di.socket_address.port = port,
        }
    }
    pub fn to_socket_addr(&self) -> SocketAddr {
        match self {
            Self::UDP(di) => di.socket_address.to_socket_addr(),
            Self::TCP(di) => di.socket_address.to_socket_addr(),
            Self::WS(di) => di.socket_address.to_socket_addr(),
            Self::WSS(di) => di.socket_address.to_socket_addr(),
        }
    }
    pub fn to_peer_address(&self) -> PeerAddress {
        match self {
            Self::UDP(di) => PeerAddress::new(di.socket_address, ProtocolType::UDP),
            Self::TCP(di) => PeerAddress::new(di.socket_address, ProtocolType::TCP),
            Self::WS(di) => PeerAddress::new(di.socket_address, ProtocolType::WS),
            Self::WSS(di) => PeerAddress::new(di.socket_address, ProtocolType::WSS),
        }
    }
    pub fn request(&self) -> Option<String> {
        match self {
            Self::UDP(_) => None,
            Self::TCP(_) => None,
            Self::WS(di) => Some(format!("ws://{}", di.request)),
            Self::WSS(di) => Some(format!("wss://{}", di.request)),
        }
    }
    pub fn is_global(&self) -> bool {
        self.socket_address().address().is_global()
    }
    pub fn is_local(&self) -> bool {
        self.socket_address().address().is_local()
    }
    pub fn is_valid(&self) -> bool {
        let socket_address = self.socket_address();
        let address = socket_address.address();
        let port = socket_address.port();
        (address.is_global() || address.is_local()) && port > 0
    }
    pub fn matches_peer_scope(&self, scope: PeerScope) -> bool {
        match scope {
            PeerScope::All => true,
            PeerScope::Global => self.is_global(),
            PeerScope::Local => self.is_local(),
        }
    }
    pub fn make_filter(&self, scoped: bool) -> DialInfoFilter {
        DialInfoFilter {
            peer_scope: if scoped {
                if self.is_global() {
                    PeerScope::Global
                } else if self.is_local() {
                    PeerScope::Local
                } else {
                    PeerScope::All
                }
            } else {
                PeerScope::All
            },
            protocol_type: Some(self.protocol_type()),
            address_type: Some(self.address_type()),
        }
    }
}

impl MatchesDialInfoFilter for DialInfo {
    fn matches_filter(&self, filter: &DialInfoFilter) -> bool {
        if !self.matches_peer_scope(filter.peer_scope) {
            return false;
        }
        if let Some(pt) = filter.protocol_type {
            if self.protocol_type() != pt {
                return false;
            }
        }
        if let Some(at) = filter.address_type {
            if self.address_type() != at {
                return false;
            }
        }
        true
    }
}

//////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum PeerScope {
    All,
    Global,
    Local,
}
impl Default for PeerScope {
    fn default() -> Self {
        PeerScope::All
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub node_info: NodeInfo,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct PeerAddress {
    pub socket_address: SocketAddress,
    pub protocol_type: ProtocolType,
}

impl PeerAddress {
    pub fn new(socket_address: SocketAddress, protocol_type: ProtocolType) -> Self {
        Self {
            socket_address: socket_address.to_canonical(),
            protocol_type,
        }
    }

    pub fn to_socket_addr(&self) -> SocketAddr {
        self.socket_address.to_socket_addr()
    }

    pub fn address_type(&self) -> AddressType {
        self.socket_address.address_type()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConnectionDescriptor {
    pub remote: PeerAddress,
    pub local: Option<SocketAddress>,
}

impl ConnectionDescriptor {
    pub fn new(remote: PeerAddress, local: SocketAddress) -> Self {
        Self {
            remote,
            local: Some(local),
        }
    }
    pub fn new_no_local(remote: PeerAddress) -> Self {
        Self {
            remote,
            local: None,
        }
    }
    pub fn protocol_type(&self) -> ProtocolType {
        self.remote.protocol_type
    }
    pub fn address_type(&self) -> AddressType {
        self.remote.address_type()
    }
    pub fn matches_peer_scope(&self, scope: PeerScope) -> bool {
        match scope {
            PeerScope::All => true,
            PeerScope::Global => self.remote.socket_address.address().is_global(),
            PeerScope::Local => self.remote.socket_address.address().is_local(),
        }
    }
}

impl MatchesDialInfoFilter for ConnectionDescriptor {
    fn matches_filter(&self, filter: &DialInfoFilter) -> bool {
        if !self.matches_peer_scope(filter.peer_scope) {
            return false;
        }
        if let Some(pt) = filter.protocol_type {
            if self.protocol_type() != pt {
                return false;
            }
        }
        if let Some(at) = filter.address_type {
            if self.address_type() != at {
                return false;
            }
        }
        true
    }
}

//////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeDialInfo {
    pub node_id: NodeId,
    pub dial_info: DialInfo,
}

impl fmt::Display for NodeDialInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}@{}", self.node_id, self.dial_info)
    }
}

impl FromStr for NodeDialInfo {
    type Err = VeilidAPIError;
    fn from_str(s: &str) -> Result<NodeDialInfo, VeilidAPIError> {
        // split out node id from the dial info
        let (node_id_str, rest) = s
            .split_once('@')
            .ok_or_else(|| parse_error!("NodeDialInfo::from_str missing @ node id separator", s))?;

        // parse out node id
        let node_id = NodeId::new(DHTKey::try_decode(node_id_str).map_err(|e| {
            parse_error!(
                format!("NodeDialInfo::from_str couldn't parse node id: {}", e),
                s
            )
        })?);
        // parse out dial info
        let dial_info = DialInfo::from_str(rest)?;

        // return completed NodeDialInfo
        Ok(NodeDialInfo { node_id, dial_info })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    pub fastest: u64, // fastest latency in the ROLLING_LATENCIES_SIZE last latencies
    pub average: u64, // average latency over the ROLLING_LATENCIES_SIZE last latencies
    pub slowest: u64, // slowest latency in the ROLLING_LATENCIES_SIZE last latencies
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TransferStats {
    pub total: u64,   // total amount transferred ever
    pub maximum: u64, // maximum rate over the ROLLING_TRANSFERS_SIZE last amounts
    pub average: u64, // average rate over the ROLLING_TRANSFERS_SIZE last amounts
    pub minimum: u64, // minimum rate over the ROLLING_TRANSFERS_SIZE last amounts
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TransferStatsDownUp {
    pub down: TransferStats,
    pub up: TransferStats,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PingStats {
    pub in_flight: u32,         // number of pings issued that have yet to be answered
    pub total_sent: u32,        // number of pings that have been sent in the total_time range
    pub total_returned: u32, // number of pings that have been returned by the node in the total_time range
    pub consecutive_pongs: u32, // number of pongs that have been received and returned consecutively without a lost ping
    pub last_pinged: Option<u64>, // when the peer was last pinged
    pub first_consecutive_pong_time: Option<u64>, // the timestamp of the first pong in a series of consecutive pongs
    pub recent_lost_pings: u32, // number of pings that have been lost since we lost reliability
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeerStats {
    pub time_added: u64,               // when the peer was added to the routing table
    pub last_seen: Option<u64>, // when the peer was last seen for any reason, including when we first attempted to reach out to it
    pub ping_stats: PingStats,  // information about pings
    pub latency: Option<LatencyStats>, // latencies for communications with the peer
    pub transfer: TransferStatsDownUp, // Stats for communications with the peer
    pub status: Option<NodeStatus>, // Last known node status
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

#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub enum TunnelMode {
    Raw,
    Turn,
}

type TunnelId = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FullTunnel {
    pub id: TunnelId,
    pub timeout: u64,
    pub local: TunnelEndpoint,
    pub remote: TunnelEndpoint,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PartialTunnel {
    pub id: TunnelId,
    pub timeout: u64,
    pub local: TunnelEndpoint,
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RouteHopSpec {
    pub dial_info: NodeDialInfo,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrivateRouteSpec {
    //
    pub public_key: DHTKey,
    pub secret_key: DHTKeySecret,
    pub hops: Vec<RouteHopSpec>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoutingContextOptions {
    pub safety_route_spec: Option<SafetyRouteSpec>,
    pub private_route_spec: Option<PrivateRouteSpec>,
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
            inner: Arc::new(Mutex::new(RoutingContextInner { api, options })),
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
    context: Option<VeilidCoreContext>,
}

impl fmt::Debug for VeilidAPIInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VeilidAPIInner")
    }
}

impl Drop for VeilidAPIInner {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            intf::spawn_local(api_shutdown(context)).detach();
        }
    }
}

#[derive(Clone, Debug)]
pub struct VeilidAPI {
    inner: Arc<Mutex<VeilidAPIInner>>,
}

impl VeilidAPI {
    pub(crate) fn new(context: VeilidCoreContext) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VeilidAPIInner {
                context: Some(context),
            })),
        }
    }

    pub async fn shutdown(self) {
        let context = { self.inner.lock().context.take() };
        if let Some(context) = context {
            api_shutdown(context).await;
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.inner.lock().context.is_none()
    }

    ////////////////////////////////////////////////////////////////
    // Accessors
    pub fn config(&self) -> Result<VeilidConfig, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.config.clone());
        }
        Err(VeilidAPIError::NotInitialized)
    }
    pub fn crypto(&self) -> Result<Crypto, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.crypto.clone());
        }
        Err(VeilidAPIError::NotInitialized)
    }
    pub fn table_store(&self) -> Result<TableStore, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.table_store.clone());
        }
        Err(VeilidAPIError::NotInitialized)
    }
    pub fn block_store(&self) -> Result<BlockStore, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.block_store.clone());
        }
        Err(VeilidAPIError::NotInitialized)
    }
    pub fn protected_store(&self) -> Result<ProtectedStore, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.protected_store.clone());
        }
        Err(VeilidAPIError::NotInitialized)
    }
    pub fn attachment_manager(&self) -> Result<AttachmentManager, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.attachment_manager.clone());
        }
        Err(VeilidAPIError::NotInitialized)
    }
    pub fn network_manager(&self) -> Result<NetworkManager, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.attachment_manager.network_manager());
        }
        Err(VeilidAPIError::NotInitialized)
    }
    pub fn rpc_processor(&self) -> Result<RPCProcessor, VeilidAPIError> {
        let inner = self.inner.lock();
        if let Some(context) = &inner.context {
            return Ok(context.attachment_manager.network_manager().rpc_processor());
        }
        Err(VeilidAPIError::NotInitialized)
    }

    ////////////////////////////////////////////////////////////////
    // Attach/Detach

    // get a full copy of the current state
    pub async fn get_state(&self) -> Result<VeilidState, VeilidAPIError> {
        let attachment_manager = self.attachment_manager()?;
        Ok(VeilidState {
            attachment: attachment_manager.get_state(),
        })
    }

    // connect to the network
    pub async fn attach(&self) -> Result<(), VeilidAPIError> {
        let attachment_manager = self.attachment_manager()?;
        attachment_manager
            .request_attach()
            .await
            .map_err(|e| VeilidAPIError::Internal { message: e })
    }

    // disconnect from the network
    pub async fn detach(&self) -> Result<(), VeilidAPIError> {
        let attachment_manager = self.attachment_manager()?;
        attachment_manager
            .request_detach()
            .await
            .map_err(|e| VeilidAPIError::Internal { message: e })
    }

    // Change api logging level if it is enabled
    pub async fn change_log_level(&self, log_level: VeilidConfigLogLevel) {
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                set_max_level(log_level.to_level_filter());
            } else {
                use api_logger::ApiLogger;
                ApiLogger::change_log_level(log_level.to_level_filter());
            }
        }
    }

    ////////////////////////////////////////////////////////////////
    // Direct Node Access (pretty much for testing only)

    pub async fn info(&self, node_id: NodeId) -> Result<InfoAnswer, VeilidAPIError> {
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        let node_ref = match routing_table.lookup_node_ref(node_id.key) {
            None => return Err(VeilidAPIError::NodeNotFound { node_id }),
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
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        let node_ref = match routing_table.lookup_node_ref(node_id.key) {
            None => return Err(VeilidAPIError::NodeNotFound { node_id }),
            Some(nr) => nr,
        };
        rpc.rpc_call_validate_dial_info(node_ref.clone(), dial_info, redirect, alternate_port)
            .await
            .map_err(map_rpc_error!())
    }

    pub async fn search_dht(&self, node_id: NodeId) -> Result<SearchDHTAnswer, VeilidAPIError> {
        let rpc_processor = self.rpc_processor()?;
        let config = self.config()?;
        let (count, fanout, timeout) = {
            let c = config.get();
            (
                c.network.dht.resolve_node_count,
                c.network.dht.resolve_node_fanout,
                c.network.dht.resolve_node_timeout_ms.map(ms_to_us),
            )
        };

        let node_ref = rpc_processor
            .search_dht_single_key(node_id.key, count, fanout, timeout)
            .await
            .map_err(map_rpc_error!())?;

        let answer = node_ref.operate(|e| SearchDHTAnswer {
            node_id: NodeId::new(node_ref.node_id()),
            dial_info: e.dial_infos().to_vec(),
        });

        Ok(answer)
    }

    pub async fn search_dht_multi(
        &self,
        node_id: NodeId,
    ) -> Result<Vec<SearchDHTAnswer>, VeilidAPIError> {
        let rpc_processor = self.rpc_processor()?;
        let config = self.config()?;
        let (count, fanout, timeout) = {
            let c = config.get();
            (
                c.network.dht.resolve_node_count,
                c.network.dht.resolve_node_fanout,
                c.network.dht.resolve_node_timeout_ms.map(ms_to_us),
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
                dial_info: e.dial_infos().to_vec(),
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
        panic!("unimplemented");
    }

    pub async fn complete_tunnel(
        &self,
        _endpoint_mode: TunnelMode,
        _depth: u8,
        _partial_tunnel: PartialTunnel,
    ) -> Result<FullTunnel, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn cancel_tunnel(&self, _tunnel_id: TunnelId) -> Result<bool, VeilidAPIError> {
        panic!("unimplemented");
    }
}

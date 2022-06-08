#![allow(dead_code)]

mod debug;
mod serialize_helpers;
pub use debug::*;
pub use serialize_helpers::*;

use crate::*;

pub use crate::xx::{
    IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, SystemPinBoxFuture,
    ToSocketAddrs,
};
pub use alloc::string::ToString;
pub use attachment_manager::AttachmentManager;
pub use core::str::FromStr;
pub use dht::Crypto;
pub use dht::{generate_secret, sign, verify, DHTKey, DHTKeySecret, DHTSignature};
pub use intf::BlockStore;
pub use intf::ProtectedStore;
pub use intf::TableStore;
pub use network_manager::NetworkManager;
pub use routing_table::RoutingTable;
pub use rpc_processor::StatusAnswer;

use api_tracing_layer::*;
use core::fmt;
use core_context::{api_shutdown, VeilidCoreContext};
use enumset::*;
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
    NoPeerInfo {
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
            VeilidAPIError::NoPeerInfo { node_id } => {
                write!(f, "VeilidAPIError::NoPeerInfo({})", node_id)
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
        RPCError::Unreachable(n) => VeilidAPIError::NodeNotFound {
            node_id: NodeId::new(n),
        },
        RPCError::Unimplemented(s) => VeilidAPIError::Unimplemented { message: s },
        RPCError::Internal(s) => VeilidAPIError::Internal { message: s },
        RPCError::Protocol(s) => VeilidAPIError::Internal { message: s },
        RPCError::InvalidFormat(s) => VeilidAPIError::Internal {
            message: format!("Invalid RPC format: {}", s),
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy, Serialize, Deserialize)]
pub enum VeilidLogLevel {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

impl VeilidLogLevel {
    pub fn from_tracing_level(level: tracing::Level) -> VeilidLogLevel {
        match level {
            tracing::Level::ERROR => VeilidLogLevel::Error,
            tracing::Level::WARN => VeilidLogLevel::Warn,
            tracing::Level::INFO => VeilidLogLevel::Info,
            tracing::Level::DEBUG => VeilidLogLevel::Debug,
            tracing::Level::TRACE => VeilidLogLevel::Trace,
        }
    }
    pub fn from_log_level(level: log::Level) -> VeilidLogLevel {
        match level {
            log::Level::Error => VeilidLogLevel::Error,
            log::Level::Warn => VeilidLogLevel::Warn,
            log::Level::Info => VeilidLogLevel::Info,
            log::Level::Debug => VeilidLogLevel::Debug,
            log::Level::Trace => VeilidLogLevel::Trace,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeilidStateLog {
    pub log_level: VeilidLogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeilidStateAttachment {
    pub state: AttachmentState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeilidStateNetwork {
    pub started: bool,
    pub bps_down: u64,
    pub bps_up: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VeilidUpdate {
    Log(VeilidStateLog),
    Attachment(VeilidStateAttachment),
    Network(VeilidStateNetwork),
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeilidState {
    pub attachment: VeilidStateAttachment,
    pub network: VeilidStateNetwork,
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

// Keep member order appropriate for sorting < preference
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum DialInfoClass {
    Direct = 0, // D = Directly reachable with public IP and no firewall, with statically configured port
    Mapped = 1, // M = Directly reachable with via portmap behind any NAT or firewalled with dynamically negotiated port
    FullConeNAT = 2, // F = Directly reachable device without portmap behind full-cone NAT
    Blocked = 3, // B = Inbound blocked at firewall but may hole punch with public address
    AddressRestrictedNAT = 4, // A = Device without portmap behind address-only restricted NAT
    PortRestrictedNAT = 5, // P = Device without portmap behind address-and-port restricted NAT
}

impl DialInfoClass {
    // Is a signal required to do an inbound hole-punch?
    pub fn requires_signal(&self) -> bool {
        matches!(
            self,
            Self::Blocked | Self::AddressRestrictedNAT | Self::PortRestrictedNAT
        )
    }

    // Does a relay node need to be allocated for this dial info?
    // For full cone NAT, the relay itself may not be used but the keepalive sent to it
    // is required to keep the NAT mapping valid in the router state table
    pub fn requires_relay(&self) -> bool {
        matches!(
            self,
            Self::FullConeNAT
                | Self::Blocked
                | Self::AddressRestrictedNAT
                | Self::PortRestrictedNAT
        )
    }
}

// Keep member order appropriate for sorting < preference
#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
pub struct DialInfoDetail {
    pub class: DialInfoClass,
    pub dial_info: DialInfo,
}

impl MatchesDialInfoFilter for DialInfoDetail {
    fn matches_filter(&self, filter: &DialInfoFilter) -> bool {
        self.dial_info.matches_filter(filter)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum NetworkClass {
    InboundCapable = 0, // I = Inbound capable without relay, may require signal
    OutboundOnly = 1, // O = Outbound only, inbound relay required except with reverse connect signal
    WebApp = 2,       // W = PWA, outbound relay is required in most cases
    Invalid = 3,      // X = Invalid network class, we don't know how to reach this node
}

impl Default for NetworkClass {
    fn default() -> Self {
        Self::Invalid
    }
}

impl NetworkClass {
    // Should an outbound relay be kept available?
    pub fn outbound_wants_relay(&self) -> bool {
        matches!(self, Self::WebApp)
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub network_class: NetworkClass,
    pub outbound_protocols: ProtocolSet,
    pub min_version: u8,
    pub max_version: u8,
    pub dial_info_detail_list: Vec<DialInfoDetail>,
    pub relay_peer_info: Option<Box<PeerInfo>>,
}

impl NodeInfo {
    pub fn is_valid(&self) -> bool {
        !matches!(self.network_class, NetworkClass::Invalid)
    }
    pub fn first_filtered_dial_info_detail<F>(&self, filter: F) -> Option<DialInfoDetail>
    where
        F: Fn(&DialInfoDetail) -> bool,
    {
        for did in &self.dial_info_detail_list {
            if filter(did) {
                return Some(did.clone());
            }
        }
        None
    }

    pub fn all_filtered_dial_info_details<F>(&self, filter: F) -> Vec<DialInfoDetail>
    where
        F: Fn(&DialInfoDetail) -> bool,
    {
        let mut dial_info_detail_list = Vec::new();

        for did in &self.dial_info_detail_list {
            if filter(did) {
                dial_info_detail_list.push(did.clone());
            }
        }
        dial_info_detail_list
    }

    pub fn has_any_dial_info(&self) -> bool {
        !self.dial_info_detail_list.is_empty()
            || !self
                .relay_peer_info
                .as_ref()
                .map(|rpi| rpi.signed_node_info.node_info.has_direct_dial_info())
                .unwrap_or_default()
    }

    pub fn has_direct_dial_info(&self) -> bool {
        !self.dial_info_detail_list.is_empty()
    }

    // Is some relay required either for signal or inbound relay or outbound relay?
    pub fn requires_relay(&self) -> bool {
        match self.network_class {
            NetworkClass::InboundCapable => {
                for did in &self.dial_info_detail_list {
                    if did.class.requires_relay() {
                        return true;
                    }
                }
            }
            NetworkClass::OutboundOnly => {
                return true;
            }
            NetworkClass::WebApp => {
                return true;
            }
            NetworkClass::Invalid => {}
        }
        false
    }

    // Can this node assist with signalling? Yes but only if it doesn't require signalling, itself.
    pub fn can_signal(&self) -> bool {
        // Must be inbound capable
        if !matches!(self.network_class, NetworkClass::InboundCapable) {
            return false;
        }
        // Do any of our dial info require signalling? if so, we can't offer signalling
        for did in &self.dial_info_detail_list {
            if did.class.requires_signal() {
                return false;
            }
        }
        true
    }

    // Can this node relay be an inbound relay?
    pub fn can_inbound_relay(&self) -> bool {
        // For now this is the same
        self.can_signal()
    }

    // Is this node capable of validating dial info
    pub fn can_validate_dial_info(&self) -> bool {
        // For now this is the same
        self.can_signal()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalNodeInfo {
    pub dial_info_list: Vec<DialInfo>,
}

impl LocalNodeInfo {
    pub fn first_filtered_dial_info<F>(&self, filter: F) -> Option<DialInfo>
    where
        F: Fn(&DialInfo) -> bool,
    {
        for di in &self.dial_info_list {
            if filter(di) {
                return Some(di.clone());
            }
        }
        None
    }

    pub fn all_filtered_dial_info<F>(&self, filter: F) -> Vec<DialInfo>
    where
        F: Fn(&DialInfo) -> bool,
    {
        let mut dial_info_list = Vec::new();

        for di in &self.dial_info_list {
            if filter(di) {
                dial_info_list.push(di.clone());
            }
        }
        dial_info_list
    }

    pub fn has_dial_info(&self) -> bool {
        !self.dial_info_list.is_empty()
    }
}

#[allow(clippy::derive_hash_xor_eq)]
#[derive(Debug, PartialOrd, Ord, Hash, Serialize, Deserialize, EnumSetType)]
// Keep member order appropriate for sorting < preference
// Must match DialInfo order
pub enum ProtocolType {
    UDP,
    TCP,
    WS,
    WSS,
}

impl ProtocolType {
    pub fn is_connection_oriented(&self) -> bool {
        matches!(
            self,
            ProtocolType::TCP | ProtocolType::WS | ProtocolType::WSS
        )
    }
}

pub type ProtocolSet = EnumSet<ProtocolType>;

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
            Address::IPV4(v4) => ipv4addr_is_global(v4) && !ipv4addr_is_multicast(v4),
            Address::IPV6(v6) => ipv6addr_is_unicast_global(v6),
        }
    }
    pub fn is_local(&self) -> bool {
        match self {
            Address::IPV4(v4) => ipv4addr_is_private(v4) || ipv4addr_is_link_local(v4),
            Address::IPV6(v6) => {
                ipv6addr_is_unicast_site_local(v6)
                    || ipv6addr_is_unicast_link_local(v6)
                    || ipv6addr_is_unique_local(v6)
            }
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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DialInfoFilter {
    pub peer_scope: PeerScope,
    pub protocol_set: ProtocolSet,
    pub address_type: Option<AddressType>,
}

impl Default for DialInfoFilter {
    fn default() -> Self {
        Self {
            peer_scope: PeerScope::All,
            protocol_set: ProtocolSet::all(),
            address_type: None,
        }
    }
}

impl DialInfoFilter {
    pub fn all() -> Self {
        Self {
            peer_scope: PeerScope::All,
            protocol_set: ProtocolSet::all(),
            address_type: None,
        }
    }
    pub fn global() -> Self {
        Self {
            peer_scope: PeerScope::Global,
            protocol_set: ProtocolSet::all(),
            address_type: None,
        }
    }
    pub fn local() -> Self {
        Self {
            peer_scope: PeerScope::Local,
            protocol_set: ProtocolSet::all(),
            address_type: None,
        }
    }
    pub fn scoped(peer_scope: PeerScope) -> Self {
        Self {
            peer_scope,
            protocol_set: ProtocolSet::all(),
            address_type: None,
        }
    }
    pub fn with_protocol_type(mut self, protocol_type: ProtocolType) -> Self {
        self.protocol_set = ProtocolSet::only(protocol_type);
        self
    }
    pub fn with_protocol_set(mut self, protocol_set: ProtocolSet) -> Self {
        self.protocol_set = protocol_set;
        self
    }
    pub fn with_address_type(mut self, address_type: AddressType) -> Self {
        self.address_type = Some(address_type);
        self
    }
}

impl fmt::Debug for DialInfoFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let mut out = String::new();
        out += &format!("{:?}", self.peer_scope);
        if self.protocol_set != ProtocolSet::all() {
            out += &format!("+{:?}", self.protocol_set);
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
// Keep member order appropriate for sorting < preference
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
            DialInfo::WS(di) => {
                let url = format!("ws://{}", di.request);
                let split_url = SplitUrl::from_str(&url).unwrap();
                match split_url.host {
                    SplitUrlHost::Hostname(_) => {
                        write!(f, "ws|{}|{}", di.socket_address.to_ip_addr(), di.request)
                    }
                    SplitUrlHost::IpAddr(a) => {
                        if di.socket_address.to_ip_addr() == a {
                            write!(f, "ws|{}", di.request)
                        } else {
                            panic!("resolved address does not match url: {}", di.request);
                        }
                    }
                }
            }
            DialInfo::WSS(di) => {
                let url = format!("wss://{}", di.request);
                let split_url = SplitUrl::from_str(&url).unwrap();
                match split_url.host {
                    SplitUrlHost::Hostname(_) => {
                        write!(f, "wss|{}|{}", di.socket_address.to_ip_addr(), di.request)
                    }
                    SplitUrlHost::IpAddr(_) => {
                        panic!(
                            "secure websockets can not use ip address in request: {}",
                            di.request
                        );
                    }
                }
            }
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
                let url = format!("ws://{}", rest);
                let split_url = SplitUrl::from_str(&url)
                    .map_err(|e| parse_error!(format!("unable to split WS url: {}", e), url))?;
                if split_url.scheme != "ws" || !url.starts_with("ws://") {
                    return Err(parse_error!("incorrect scheme for WS dialinfo", url));
                }
                let url_port = split_url.port.unwrap_or(80u16);

                match rest.split_once('|') {
                    Some((sa, rest)) => {
                        let address = Address::from_str(sa)?;

                        DialInfo::try_ws(
                            SocketAddress::new(address, url_port),
                            format!("ws://{}", rest),
                        )
                    }
                    None => {
                        let address = Address::from_str(&split_url.host.to_string())?;
                        DialInfo::try_ws(
                            SocketAddress::new(address, url_port),
                            format!("ws://{}", rest),
                        )
                    }
                }
            }
            "wss" => {
                let url = format!("wss://{}", rest);
                let split_url = SplitUrl::from_str(&url)
                    .map_err(|e| parse_error!(format!("unable to split WSS url: {}", e), url))?;
                if split_url.scheme != "wss" || !url.starts_with("wss://") {
                    return Err(parse_error!("incorrect scheme for WSS dialinfo", url));
                }
                let url_port = split_url.port.unwrap_or(443u16);

                let (a, rest) = rest.split_once('|').ok_or_else(|| {
                    parse_error!("DialInfo::from_str missing socket address '|' separator", s)
                })?;

                let address = Address::from_str(a)?;
                DialInfo::try_wss(
                    SocketAddress::new(address, url_port),
                    format!("wss://{}", rest),
                )
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
        if let SplitUrlHost::IpAddr(a) = split_url.host {
            if socket_address.to_ip_addr() != a {
                return Err(parse_error!(
                    format!("request address does not match socket address: {}", a),
                    socket_address
                ));
            }
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
        if !matches!(split_url.host, SplitUrlHost::Hostname(_)) {
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
            protocol_set: ProtocolSet::only(self.protocol_type()),
            address_type: Some(self.address_type()),
        }
    }

    pub fn try_vec_from_short<S: AsRef<str>, H: AsRef<str>>(
        short: S,
        hostname: H,
    ) -> Result<Vec<Self>, VeilidAPIError> {
        let short = short.as_ref();
        let hostname = hostname.as_ref();

        if short.len() < 2 {
            return Err(parse_error!("invalid short url length", short));
        }
        let url = match &short[0..1] {
            "U" => {
                format!("udp://{}:{}", hostname, &short[1..])
            }
            "T" => {
                format!("tcp://{}:{}", hostname, &short[1..])
            }
            "W" => {
                format!("ws://{}:{}", hostname, &short[1..])
            }
            "S" => {
                format!("wss://{}:{}", hostname, &short[1..])
            }
            _ => {
                return Err(parse_error!("invalid short url type", short));
            }
        };
        Self::try_vec_from_url(url)
    }

    pub fn try_vec_from_url<S: AsRef<str>>(url: S) -> Result<Vec<Self>, VeilidAPIError> {
        let url = url.as_ref();
        let split_url = SplitUrl::from_str(url)
            .map_err(|e| parse_error!(format!("unable to split url: {}", e), url))?;

        let port = match split_url.scheme.as_str() {
            "udp" | "tcp" => split_url
                .port
                .ok_or_else(|| parse_error!("Missing port in udp url", url))?,
            "ws" => split_url.port.unwrap_or(80u16),
            "wss" => split_url.port.unwrap_or(443u16),
            _ => {
                return Err(parse_error!(
                    "Invalid dial info url scheme",
                    split_url.scheme
                ));
            }
        };

        let socket_addrs = match split_url.host {
            SplitUrlHost::Hostname(_) => split_url
                .host_port(port)
                .to_socket_addrs()
                .map_err(|_| parse_error!("couldn't resolve hostname in url", url))?
                .collect(),
            SplitUrlHost::IpAddr(a) => vec![SocketAddr::new(a, port)],
        };

        let mut out = Vec::new();
        for sa in socket_addrs {
            out.push(match split_url.scheme.as_str() {
                "udp" => Self::udp_from_socketaddr(sa),
                "tcp" => Self::tcp_from_socketaddr(sa),
                "ws" => Self::try_ws(
                    SocketAddress::from_socket_addr(sa).to_canonical(),
                    url.to_string(),
                )?,
                "wss" => Self::try_wss(
                    SocketAddress::from_socket_addr(sa).to_canonical(),
                    url.to_string(),
                )?,
                _ => {
                    unreachable!("Invalid dial info url scheme")
                }
            });
        }
        Ok(out)
    }

    pub async fn to_short(&self) -> (String, String) {
        match self {
            DialInfo::UDP(di) => (
                format!("U{}", di.socket_address.port()),
                intf::ptr_lookup(di.socket_address.to_ip_addr())
                    .await
                    .unwrap_or_else(|_| di.socket_address.to_string()),
            ),
            DialInfo::TCP(di) => (
                format!("T{}", di.socket_address.port()),
                intf::ptr_lookup(di.socket_address.to_ip_addr())
                    .await
                    .unwrap_or_else(|_| di.socket_address.to_string()),
            ),
            DialInfo::WS(di) => {
                let mut split_url = SplitUrl::from_str(&format!("ws://{}", di.request)).unwrap();
                if let SplitUrlHost::IpAddr(a) = split_url.host {
                    if let Ok(host) = intf::ptr_lookup(a).await {
                        split_url.host = SplitUrlHost::Hostname(host);
                    }
                }
                (
                    format!(
                        "W{}{}",
                        split_url.port.unwrap_or(80),
                        split_url
                            .path
                            .map(|p| format!("/{}", p))
                            .unwrap_or_default()
                    ),
                    split_url.host.to_string(),
                )
            }
            DialInfo::WSS(di) => {
                let mut split_url = SplitUrl::from_str(&format!("wss://{}", di.request)).unwrap();
                if let SplitUrlHost::IpAddr(a) = split_url.host {
                    if let Ok(host) = intf::ptr_lookup(a).await {
                        split_url.host = SplitUrlHost::Hostname(host);
                    }
                }
                (
                    format!(
                        "S{}{}",
                        split_url.port.unwrap_or(443),
                        split_url
                            .path
                            .map(|p| format!("/{}", p))
                            .unwrap_or_default()
                    ),
                    split_url.host.to_string(),
                )
            }
        }
    }
    pub async fn to_url(&self) -> String {
        match self {
            DialInfo::UDP(di) => intf::ptr_lookup(di.socket_address.to_ip_addr())
                .await
                .map(|h| format!("udp://{}:{}", h, di.socket_address.port()))
                .unwrap_or_else(|_| format!("udp://{}", di.socket_address)),
            DialInfo::TCP(di) => intf::ptr_lookup(di.socket_address.to_ip_addr())
                .await
                .map(|h| format!("tcp://{}:{}", h, di.socket_address.port()))
                .unwrap_or_else(|_| format!("tcp://{}", di.socket_address)),
            DialInfo::WS(di) => {
                let mut split_url = SplitUrl::from_str(&format!("ws://{}", di.request)).unwrap();
                if let SplitUrlHost::IpAddr(a) = split_url.host {
                    if let Ok(host) = intf::ptr_lookup(a).await {
                        split_url.host = SplitUrlHost::Hostname(host);
                    }
                }
                split_url.to_string()
            }
            DialInfo::WSS(di) => {
                let mut split_url = SplitUrl::from_str(&format!("wss://{}", di.request)).unwrap();
                if let SplitUrlHost::IpAddr(a) = split_url.host {
                    if let Ok(host) = intf::ptr_lookup(a).await {
                        split_url.host = SplitUrlHost::Hostname(host);
                    }
                }
                split_url.to_string()
            }
        }
    }
}

impl MatchesDialInfoFilter for DialInfo {
    fn matches_filter(&self, filter: &DialInfoFilter) -> bool {
        if !self.matches_peer_scope(filter.peer_scope) {
            return false;
        }
        if !filter.protocol_set.contains(self.protocol_type()) {
            return false;
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

// Signed NodeInfo that can be passed around amongst peers and verifiable
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedNodeInfo {
    pub node_info: NodeInfo,
    pub signature: DHTSignature,
    pub timestamp: u64,
}

impl SignedNodeInfo {
    pub fn new(
        node_info: NodeInfo,
        node_id: NodeId,
        signature: DHTSignature,
        timestamp: u64,
    ) -> Result<Self, String> {
        let mut node_info_bytes = serde_cbor::to_vec(&node_info).map_err(map_to_string)?;
        let mut timestamp_bytes = serde_cbor::to_vec(&timestamp).map_err(map_to_string)?;

        node_info_bytes.append(&mut timestamp_bytes);

        verify(&node_id.key, &node_info_bytes, &signature)?;
        Ok(Self {
            node_info,
            signature,
            timestamp,
        })
    }

    pub fn with_secret(
        node_info: NodeInfo,
        node_id: NodeId,
        secret: &DHTKeySecret,
    ) -> Result<Self, String> {
        let timestamp = intf::get_timestamp();

        let mut node_info_bytes = serde_cbor::to_vec(&node_info).map_err(map_to_string)?;
        let mut timestamp_bytes = serde_cbor::to_vec(&timestamp).map_err(map_to_string)?;

        node_info_bytes.append(&mut timestamp_bytes);

        let signature = sign(&node_id.key, secret, &node_info_bytes)?;
        Ok(Self {
            node_info,
            signature,
            timestamp,
        })
    }

    pub fn with_no_signature(node_info: NodeInfo) -> Self {
        Self {
            node_info,
            signature: DHTSignature::default(),
            timestamp: intf::get_timestamp(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.signature.valid && self.node_info.is_valid()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub signed_node_info: SignedNodeInfo,
}

impl PeerInfo {
    pub fn new(node_id: NodeId, signed_node_info: SignedNodeInfo) -> Self {
        Self {
            node_id,
            signed_node_info,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct PeerAddress {
    socket_address: SocketAddress,
    protocol_type: ProtocolType,
}

impl PeerAddress {
    pub fn new(socket_address: SocketAddress, protocol_type: ProtocolType) -> Self {
        Self {
            socket_address: socket_address.to_canonical(),
            protocol_type,
        }
    }

    pub fn socket_address(&self) -> &SocketAddress {
        &self.socket_address
    }

    pub fn protocol_type(&self) -> ProtocolType {
        self.protocol_type
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
    remote: PeerAddress,
    local: Option<SocketAddress>,
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
    pub fn remote(&self) -> PeerAddress {
        self.remote
    }
    pub fn remote_address(&self) -> &SocketAddress {
        self.remote.socket_address()
    }
    pub fn local(&self) -> Option<SocketAddress> {
        self.local
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
        if !filter.protocol_set.contains(self.protocol_type()) {
            return false;
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
pub struct RPCStats {
    pub messages_sent: u32, // number of rpcs that have been sent in the total_time range
    pub messages_rcvd: u32, // number of rpcs that have been received in the total_time range
    pub questions_in_flight: u32, // number of questions issued that have yet to be answered
    pub last_question: Option<u64>, // when the peer was last questioned (either successfully or not) and we wanted an answer
    pub last_seen_ts: Option<u64>, // when the peer was last seen for any reason, including when we first attempted to reach out to it
    pub first_consecutive_seen_ts: Option<u64>, // the timestamp of the first consecutive proof-of-life for this node (an answer or received question)
    pub recent_lost_answers: u32, // number of answers that have been lost since we lost reliability
    pub failed_to_send: u32, // number of messages that have failed to send since we last successfully sent one
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeerStats {
    pub time_added: u64,               // when the peer was added to the routing table
    pub rpc_stats: RPCStats,           // information about RPCs
    pub latency: Option<LatencyStats>, // latencies for communications with the peer
    pub transfer: TransferStatsDownUp, // Stats for communications with the peer
    pub status: Option<NodeStatus>,    // Last known node status
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
/////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SignalInfo {
    HolePunch {
        // UDP Hole Punch Request
        receipt: Vec<u8>,    // Receipt to be returned after the hole punch
        peer_info: PeerInfo, // Sender's peer info
    },
    ReverseConnect {
        // Reverse Connection Request
        receipt: Vec<u8>,    // Receipt to be returned by the reverse connection
        peer_info: PeerInfo, // Sender's peer info
    },
    // XXX: WebRTC
    // XXX: App-level signalling
}

/////////////////////////////////////////////////////////////////////////////////////////////////////
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
        let network_manager = attachment_manager.network_manager();

        let attachment = attachment_manager.get_veilid_state();
        let network = network_manager.get_veilid_state();

        Ok(VeilidState {
            attachment,
            network,
        })
    }

    // get network connectedness

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
    pub async fn change_api_log_level(&self, log_level: VeilidConfigLogLevel) {
        ApiTracingLayer::change_api_log_level(log_level.to_veilid_log_level());
    }

    ////////////////////////////////////////////////////////////////
    // Direct Node Access (pretty much for testing only)

    pub async fn status(&self, node_id: NodeId) -> Result<StatusAnswer, VeilidAPIError> {
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        let node_ref = match routing_table.lookup_node_ref(node_id.key) {
            None => return Err(VeilidAPIError::NodeNotFound { node_id }),
            Some(nr) => nr,
        };
        let status_answer = rpc
            .rpc_call_status(node_ref)
            .await
            .map_err(map_rpc_error!())?;
        Ok(status_answer)
    }

    pub async fn validate_dial_info(
        &self,
        node_id: NodeId,
        dial_info: DialInfo,
        redirect: bool,
    ) -> Result<bool, VeilidAPIError> {
        let rpc = self.rpc_processor()?;
        let routing_table = rpc.routing_table();
        let node_ref = match routing_table.lookup_node_ref(node_id.key) {
            None => return Err(VeilidAPIError::NodeNotFound { node_id }),
            Some(nr) => nr,
        };
        rpc.rpc_call_validate_dial_info(node_ref.clone(), dial_info, redirect)
            .await
            .map_err(map_rpc_error!())
    }

    pub async fn search_dht(&self, node_id: NodeId) -> Result<PeerInfo, VeilidAPIError> {
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

        let answer = node_ref.peer_info();
        if let Some(answer) = answer {
            Ok(answer)
        } else {
            Err(VeilidAPIError::NoPeerInfo {
                node_id: NodeId::new(node_ref.node_id()),
            })
        }
    }

    pub async fn search_dht_multi(&self, node_id: NodeId) -> Result<Vec<PeerInfo>, VeilidAPIError> {
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

        let answer = node_refs.iter().filter_map(|x| x.peer_info()).collect();

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

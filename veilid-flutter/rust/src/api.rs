use flutter_rust_bridge::*;

/////////////////////////////////////////
// Config Settings
// Not all settings available through Veilid API are available to Flutter applications

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigUDP {
    pub enabled: bool,
    pub socket_pool_size: u32,
    pub listen_address: String,
    pub public_address: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigTCP {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub public_address: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigWS {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigWSS {
    pub connect: bool,
    pub max_connections: u32,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigProtocol {
    pub udp: VeilidConfigUDP,
    pub tcp: VeilidConfigTCP,
    pub ws: VeilidConfigWS,
    pub wss: VeilidConfigWSS,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigDHT {
    pub resolve_node_timeout: Option<u64>,
    pub resolve_node_count: u32,
    pub resolve_node_fanout: u32,
    pub max_find_node_count: u32,
    pub get_value_timeout: Option<u64>,
    pub get_value_count: u32,
    pub get_value_fanout: u32,
    pub set_value_timeout: Option<u64>,
    pub set_value_count: u32,
    pub set_value_fanout: u32,
    pub min_peer_count: u32,
    pub min_peer_refresh_time: u64,
    pub validate_dial_info_receipt_time: u64,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigRPC {
    pub concurrency: u32,
    pub queue_size: u32,
    pub max_timestamp_behind: Option<u64>,
    pub max_timestamp_ahead: Option<u64>,
    pub timeout: u64,
    pub max_route_hop_count: u8,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigLeases {
    pub max_server_signal_leases: u32,
    pub max_server_relay_leases: u32,
    pub max_client_signal_leases: u32,
    pub max_client_relay_leases: u32,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigNetwork {
    pub max_connections: u32,
    pub connection_initial_timeout: u64,
    pub node_id: key::DHTKey,
    pub node_id_secret: key::DHTKeySecret,
    pub bootstrap: Vec<String>,
    pub rpc: VeilidConfigRPC,
    pub dht: VeilidConfigDHT,
    pub upnp: bool,
    pub natpmp: bool,
    pub enable_local_peer_scope: bool,
    pub restricted_nat_retries: u32,
    pub tls: VeilidConfigTLS,
    pub protocol: VeilidConfigProtocol,
    pub leases: VeilidConfigLeases,
}

#[derive(Default, Clone)]
pub struct VeilidConfigTableStore {
    pub directory: String,
    pub delete: bool,
}

#[derive(Default, Clone)]
pub struct VeilidConfigBlockStore {
    pub directory: String,
    pub delete: bool,
}

#[derive(Default, Clone)]
pub struct VeilidConfigProtectedStore {
    pub allow_insecure_fallback: bool,
    pub always_use_insecure_storage: bool,
    pub insecure_fallback_directory: String,
    pub delete: bool,
}

#[derive(Default, Clone)]
pub struct VeilidConfigCapabilities {
    pub protocol_udp: bool,
    pub protocol_connect_tcp: bool,
    pub protocol_accept_tcp: bool,
    pub protocol_connect_ws: bool,
    pub protocol_accept_ws: bool,
    pub protocol_connect_wss: bool,
    pub protocol_accept_wss: bool,
}

#[derive(Default, Clone)]
pub struct VeilidConfig {
    pub program_name: String,
    pub namespace: String,
    pub capabilities: VeilidConfigCapabilities,
    pub protected_store: VeilidConfigProtectedStore,
    pub table_store: VeilidConfigTableStore,
    pub block_store: VeilidConfigBlockStore,
    pub network: VeilidConfigNetwork,
}

/////////////////////////////////////////


/////////////////////////////////////////

#[derive(Debug)]
pub enum APIErrorKind {
    AlreadyInitialized,
    NotInitialized,
    InvalidConfig,
    Timeout,
    Shutdown,
    NodeNotFound(String),
    NoDialInfo(String),
    Internal(String),
    Unimplemented(String),
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

#[derive(Debug)]
pub struct VeilidAPIError {
    kind: APIErrorKind,
    message: String,
}

#[derive(Debug)]
pub enum AttachmentState {
    Detached,
    Attaching,
    AttachedWeak,
    AttachedGood,
    AttachedStrong,
    FullyAttached,
    OverAttached,
    Detaching,
}

#[derive(Debug)]
pub enum VeilidUpdate {
    Attachment (AttachmentState),
}

/////////////////////////////////////////
/// 
pub fn startup_veilid_core(sink: StreamSink<VeilidUpdate>, config: VeilidConfig) -> Result<(), VeilidAPIError> {
    let core = veilid_core::VeilidCore::new();

    core.
}

pub fn get_veilid_state() -> Result<VeilidState, VeilidAPIError> {

}

// xxx api functions

pub fn shutdown_veilid_core() -> Result<(), VeilidAPIError> {

}

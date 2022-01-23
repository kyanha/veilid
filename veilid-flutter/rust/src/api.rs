use std::sync::Arc;
use flutter_rust_bridge::*;
use log::*;
use std::collections::HashMap;
use async_std::sync::Mutex as AsyncMutex;
use anyhow::*;
use std::fmt;

// Globals

static API: AsyncMutex<Option<veilid_core::VeilidAPI>> = AsyncMutex::new(None);

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
    pub node_id: String,
    pub node_id_secret: String,
    pub bootstrap: Vec<String>,
    pub rpc: VeilidConfigRPC,
    pub dht: VeilidConfigDHT,
    pub upnp: bool,
    pub natpmp: bool,
    pub enable_local_peer_scope: bool,
    pub restricted_nat_retries: u32,
    pub protocol: VeilidConfigProtocol,
    pub leases: VeilidConfigLeases,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigTableStore {
    pub directory: String,
    pub delete: bool,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigBlockStore {
    pub directory: String,
    pub delete: bool,
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfigProtectedStore {
  
}

#[derive(Debug, Default, Clone)]
pub struct VeilidConfig {
    pub program_name: String,
    pub namespace: String,
    // Capabilities
    pub capabilities__protocol_udp: bool,
    pub capabilities__protocol_connect_tcp: bool,
    pub capabilities__protocol_accept_tcp: bool,
    pub capabilities__protocol_connect_ws: bool,
    pub capabilities__protocol_accept_ws: bool,
    pub capabilities__protocol_connect_wss: bool,
    pub capabilities__protocol_accept_wss: bool,
    // Protected Store
    pub protected_store__allow_insecure_fallback: bool,
    pub protected_store__always_use_insecure_storage: bool,
    pub protected_store__insecure_fallback_directory: String,
    pub protected_store__delete: bool,
    // Table Store
    pub table_store: VeilidConfigTableStore,
    // Block Store
    pub block_store: VeilidConfigBlockStore,
    // Network
    pub network: VeilidConfigNetwork,
}

/////////////////////////////////////////


/////////////////////////////////////////

#[derive(Debug)]
pub enum VeilidAPIError {
    AlreadyInitialized,
    NotInitialized,
    InvalidConfig(String),
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

impl fmt::Display for VeilidAPIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VeilidAPIError::AlreadyInitialized => write!(f, "VeilidAPIError::AlreadyInitialized"),
            VeilidAPIError::NotInitialized => write!(f, "VeilidAPIError::NotInitialized"),
            VeilidAPIError::InvalidConfig(e) => write!(f, "VeilidAPIError::InvalidConfig({})", e),
            VeilidAPIError::Timeout => write!(f, "VeilidAPIError::Timeout"),
            VeilidAPIError::Shutdown => write!(f, "VeilidAPIError::Shutdown"),
            VeilidAPIError::NodeNotFound(ni) => write!(f, "VeilidAPIError::NodeNotFound({})", ni),
            VeilidAPIError::NoDialInfo(ni) => write!(f, "VeilidAPIError::NoDialInfo({})", ni),
            VeilidAPIError::Internal(e) => write!(f, "VeilidAPIError::Internal({})", e),
            VeilidAPIError::Unimplemented(e) => write!(f, "VeilidAPIError::Unimplemented({})", e),
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

impl std::error::Error for VeilidAPIError {}

impl VeilidAPIError {
    fn from_core(api_error: veilid_core::VeilidAPIError) -> Self {
        match api_error {
            veilid_core::VeilidAPIError::Timeout => VeilidAPIError::Timeout,
            veilid_core::VeilidAPIError::Shutdown => VeilidAPIError::Shutdown,
            veilid_core::VeilidAPIError::NodeNotFound(node_id) => VeilidAPIError::NodeNotFound(format!("{}",node_id)),
            veilid_core::VeilidAPIError::NoDialInfo(node_id) => VeilidAPIError::NodeNotFound(format!("{}",node_id)),
            veilid_core::VeilidAPIError::Internal(msg) => VeilidAPIError::Internal(msg.clone()),
            veilid_core::VeilidAPIError::Unimplemented(msg)=> VeilidAPIError::Unimplemented(msg.clone()),
            veilid_core::VeilidAPIError::ParseError{message, value} => VeilidAPIError::ParseError{ message: message.clone(), value: value.clone() },
            veilid_core::VeilidAPIError::InvalidArgument { context, argument, value } => VeilidAPIError::InvalidArgument{ context: context.clone(), argument: argument.clone(), value: value.clone() },
            veilid_core::VeilidAPIError::MissingArgument {context, argument }  => VeilidAPIError::MissingArgument{ context: context.clone(), argument: argument.clone() },
        }
    }
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

impl AttachmentState {
    fn from_core(attachment_state: veilid_core::AttachmentState) -> Self {
        match attachment_state {
            veilid_core::AttachmentState::Detached => AttachmentState::Detached,
            veilid_core::AttachmentState::Attaching=> AttachmentState::Attaching,
            veilid_core::AttachmentState::AttachedWeak=> AttachmentState::AttachedWeak,
            veilid_core::AttachmentState::AttachedGood=> AttachmentState::AttachedGood,
            veilid_core::AttachmentState::AttachedStrong=> AttachmentState::AttachedStrong,
            veilid_core::AttachmentState::FullyAttached=> AttachmentState::FullyAttached,
            veilid_core::AttachmentState::OverAttached=> AttachmentState::OverAttached,
            veilid_core::AttachmentState::Detaching=> AttachmentState::Detaching,
        }
    }
}


#[derive(Debug)]
pub enum VeilidUpdate {
    Attachment (AttachmentState),
}

impl VeilidUpdate {
    fn from_core(veilid_update: veilid_core::VeilidUpdate) -> Self {
        match veilid_update {
            veilid_core::VeilidUpdate::Attachment(attachment) => Self::Attachment(AttachmentState::from_core(attachment))
        }
    }
}


#[derive(Debug)]
pub struct VeilidState {
    pub attachment: AttachmentState,
}

impl VeilidState {
    fn from_core(veilid_state: veilid_core::VeilidState) -> Self {
        Self {
            attachment: AttachmentState::from_core(veilid_state.attachment)
        }
    }
}

/////////////////////////////////////////
pub fn startup_veilid_core(sink: StreamSink<VeilidUpdate>, config: VeilidConfig) -> Result<VeilidState> {
    async_std::task::block_on( async {

        let api = API.lock().await;
        if api.is_some() {
            return Err(anyhow!(VeilidAPIError::AlreadyInitialized));
        }
    
        let core = veilid_core::VeilidCore::new();
        
        // convert config to hashmap
        let config_map = HashMap::<String, Box<dyn core::any::Any + Send + 'static>>::new();
        macro_rules! get_config {
            ($key:expr) => {
                config_map.insert(stringify!($key)[7..].to_owned(), Box::new($key.clone()));
            }
        }
        macro_rules! default_config {
            ($key:expr, $default_value:expr) => {
                config_map.insert(stringify!($key)[7..].to_owned(), Box::new($default_value));
            }
        }
        get_config!(config.program_name);
        get_config!(config.namespace);
        get_config!(config.capabilities.protocol_udp);
        get_config!(config.capabilities.protocol_connect_tcp);
        get_config!(config.capabilities.protocol_accept_tcp);
        get_config!(config.capabilities.protocol_connect_ws);
        get_config!(config.capabilities.protocol_accept_ws);
        get_config!(config.capabilities.protocol_connect_wss);
        get_config!(config.capabilities.protocol_accept_wss);
        get_config!(config.table_store.directory);
        get_config!(config.table_store.delete);
        get_config!(config.block_store.directory);
        get_config!(config.block_store.delete);
        get_config!(config.protected_store.allow_insecure_fallback);
        get_config!(config.protected_store.always_use_insecure_storage);
        get_config!(config.protected_store.insecure_fallback_directory);
        get_config!(config.protected_store.delete);
        get_config!(config.network.node_id);
        get_config!(config.network.node_id_secret);
        get_config!(config.network.max_connections);
        get_config!(config.network.connection_initial_timeout);
        get_config!(config.network.bootstrap);
        get_config!(config.network.dht.resolve_node_timeout);
        get_config!(config.network.dht.resolve_node_count);
        get_config!(config.network.dht.resolve_node_fanout);
        get_config!(config.network.dht.max_find_node_count);
        get_config!(config.network.dht.get_value_timeout);
        get_config!(config.network.dht.get_value_count);
        get_config!(config.network.dht.get_value_fanout);
        get_config!(config.network.dht.set_value_timeout);
        get_config!(config.network.dht.set_value_count);
        get_config!(config.network.dht.set_value_fanout);
        get_config!(config.network.dht.min_peer_count);
        get_config!(config.network.dht.min_peer_refresh_time);
        get_config!(config.network.dht.validate_dial_info_receipt_time);
        get_config!(config.network.rpc.concurrency);
        get_config!(config.network.rpc.queue_size);
        get_config!(config.network.rpc.max_timestamp_behind);
        get_config!(config.network.rpc.max_timestamp_ahead);
        get_config!(config.network.rpc.timeout);
        get_config!(config.network.rpc.max_route_hop_count);
        get_config!(config.network.upnp);
        get_config!(config.network.natpmp);
        get_config!(config.network.enable_local_peer_scope);
        get_config!(config.network.restricted_nat_retries);
        default_config!(config.network.tls.certificate_path, "");
        default_config!(config.network.tls.private_key_path, "");
        default_config!(config.network.tls.connection_initial_timeout, 0u64);
        default_config!(config.network.application.https.enabled, false);
        default_config!(config.network.application.https.listen_address, "");
        default_config!(config.network.application.https.path, "");
        default_config!(config.network.application.https.url, Option::<String>::None);
        default_config!(config.network.application.http.enabled, false);
        default_config!(config.network.application.http.listen_address, "");
        default_config!(config.network.application.http.path, "");
        default_config!(config.network.application.http.url, Option::<String>::None);
        get_config!(config.network.protocol.udp.enabled);
        get_config!(config.network.protocol.udp.socket_pool_size);
        get_config!(config.network.protocol.udp.listen_address);
        get_config!(config.network.protocol.udp.public_address);
        get_config!(config.network.protocol.tcp.connect);
        get_config!(config.network.protocol.tcp.listen);
        get_config!(config.network.protocol.tcp.max_connections);
        get_config!(config.network.protocol.tcp.listen_address);
        get_config!(config.network.protocol.tcp.public_address);
        get_config!(config.network.protocol.ws.connect);
        get_config!(config.network.protocol.ws.listen);
        get_config!(config.network.protocol.ws.max_connections);
        get_config!(config.network.protocol.ws.listen_address);
        get_config!(config.network.protocol.ws.path);
        get_config!(config.network.protocol.ws.url);
        get_config!(config.network.protocol.wss.connect);
        default_config!(config.network.protocol.wss.listen, false);
        get_config!(config.network.protocol.wss.max_connections);
        default_config!(config.network.protocol.wss.listen_address, "");
        default_config!(config.network.protocol.wss.path, "");
        default_config!(config.network.protocol.wss.url, Option::<String>::None);
        get_config!(config.network.leases.max_server_signal_leases);
        get_config!(config.network.leases.max_server_relay_leases);
        get_config!(config.network.leases.max_client_signal_leases);
        get_config!(config.network.leases.max_client_relay_leases);

        let setup = veilid_core::VeilidCoreSetup {
            update_callback: Arc::new(
                move |update: veilid_core::VeilidUpdate| -> veilid_core::SystemPinBoxFuture<()> {
                    Box::pin(async move {
                        if !sink.add(VeilidUpdate::from_core(update)) {
                            error!("error sending veilid update callback");
                        }
                    })
                },
            ),        
            config_callback: Arc::new(
                move |key| {
                    config_map.get(&key).ok_or_else(|| {
                        let err = format!("config key '{}' doesn't exist", key);
                        error!("{}",err);
                        err
                    }).map(|v| {
                        *v.clone()
                    })
                }
            ),
        }; 

        let veilid_api = core.startup(setup).await.map_err(|e| VeilidAPIError::InvalidConfig(e.clone()))?;
        *api = Some(veilid_api.clone());
            
        let core_state = veilid_api.get_state().await.map_err(VeilidAPIError::from_core)?;
        Ok(VeilidState::from_core(core_state))
    })
}

pub fn get_veilid_state() -> Result<VeilidState> {
    async_std::task::block_on( async {
        let veilid_api = API.lock().await.ok_or(anyhow!(VeilidAPIError::NotInitialized))?;
        let core_state = veilid_api.get_state().await.map_err(VeilidAPIError::from_core)?;
        Ok(VeilidState::from_core(core_state))
    })
}

// xxx api functions

pub fn shutdown_veilid_core() -> Result<()> {
    async_std::task::block_on( async {
        let veilid_api = API.lock().await.take().ok_or(anyhow!(VeilidAPIError::NotInitialized))?;
        veilid_api.shutdown().await;        
        Ok(())
    })
}

use anyhow::*;
use async_std::sync::Mutex as AsyncMutex;
use flutter_rust_bridge::*;
use log::*;
use std::fmt;
use std::sync::Arc;

// Globals

static VEILID_API: AsyncMutex<Option<veilid_core::VeilidAPI>> = AsyncMutex::new(None);
async fn get_veilid_api() -> Result<veilid_core::VeilidAPI> {
    let api_lock = VEILID_API.lock().await;
    let veilid_api = match &*api_lock {
        None => {
            return Err(anyhow!(VeilidAPIError::NotInitialized));
        }
        Some(api) => api.clone(),
    };
    Ok(veilid_api)
}
async fn take_veilid_api() -> Result<veilid_core::VeilidAPI> {
    let mut api_lock = VEILID_API.lock().await;
    let veilid_api = match api_lock.take() {
        None => {
            return Err(anyhow!(VeilidAPIError::NotInitialized));
        }
        Some(api) => api,
    };
    Ok(veilid_api)
}

/////////////////////////////////////////
// Config Settings
// Not all settings available through Veilid API are available to Flutter applications

#[derive(Debug, Default, Clone)]
#[allow(non_snake_case)]
pub struct VeilidConfig {
    pub program_name: String,
    pub veilid_namespace: String,
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
    pub table_store__directory: String,
    pub table_store__delete: bool,
    // Block Store
    pub block_store__directory: String,
    pub block_store__delete: bool,
    // Network
    pub network__max_connections: u32,
    pub network__connection_initial_timeout_ms: u32,
    pub network__node_id: String,
    pub network__node_id_secret: String,
    pub network__bootstrap: Vec<String>,
    pub network__upnp: bool,
    pub network__natpmp: bool,
    pub network__enable_local_peer_scope: bool,
    pub network__restricted_nat_retries: u32,
    // Network / RPC
    pub network__rpc__concurrency: u32,
    pub network__rpc__queue_size: u32,
    pub network__rpc__max_timestamp_behind_ms: Option<u32>,
    pub network__rpc__max_timestamp_ahead_ms: Option<u32>,
    pub network__rpc__timeout_ms: u32,
    pub network__rpc__max_route_hop_count: u8,
    // Network / DHT
    pub network__dht__resolve_node_timeout_ms: Option<u32>,
    pub network__dht__resolve_node_count: u32,
    pub network__dht__resolve_node_fanout: u32,
    pub network__dht__max_find_node_count: u32,
    pub network__dht__get_value_timeout_ms: Option<u32>,
    pub network__dht__get_value_count: u32,
    pub network__dht__get_value_fanout: u32,
    pub network__dht__set_value_timeout_ms: Option<u32>,
    pub network__dht__set_value_count: u32,
    pub network__dht__set_value_fanout: u32,
    pub network__dht__min_peer_count: u32,
    pub network__dht__min_peer_refresh_time_ms: u32,
    pub network__dht__validate_dial_info_receipt_time_ms: u32,
    // Network / Protocol
    // Network / Protocol / UDP
    pub network__protocol__udp__enabled: bool,
    pub network__protocol__udp__socket_pool_size: u32,
    pub network__protocol__udp__listen_address: String,
    pub network__protocol__udp__public_address: Option<String>,
    // Network / Protocol / TCP
    pub network__protocol__tcp__connect: bool,
    pub network__protocol__tcp__listen: bool,
    pub network__protocol__tcp__max_connections: u32,
    pub network__protocol__tcp__listen_address: String,
    pub network__protocol__tcp__public_address: Option<String>,
    // Network / Protocol / WS
    pub network__protocol__ws__connect: bool,
    pub network__protocol__ws__listen: bool,
    pub network__protocol__ws__max_connections: u32,
    pub network__protocol__ws__listen_address: String,
    pub network__protocol__ws__path: String,
    pub network__protocol__ws__url: Option<String>,
    // Network / Protocol / WSS
    pub network__protocol__wss__connect: bool,
    pub network__protocol__wss__max_connections: u32,
    // Network / Leases
    pub network__leases__max_server_signal_leases: u32,
    pub network__leases__max_server_relay_leases: u32,
    pub network__leases__max_client_signal_leases: u32,
    pub network__leases__max_client_relay_leases: u32,
}

impl VeilidConfig {
    pub fn get_by_str(
        &self,
        key: &str,
    ) -> std::result::Result<Box<dyn std::any::Any + Send + 'static>, String> {
        let out: Box<dyn core::any::Any + Send> = match key {
            "program_name" => Box::new(self.program_name.clone()),
            "namespace" => Box::new(self.veilid_namespace.clone()),
            "capabilities.protocol_udp" => Box::new(self.capabilities__protocol_udp.clone()),
            "capabilities.protocol_connect_tcp" => {
                Box::new(self.capabilities__protocol_connect_tcp.clone())
            }
            "capabilities.protocol_accept_tcp" => {
                Box::new(self.capabilities__protocol_accept_tcp.clone())
            }
            "capabilities.protocol_connect_ws" => {
                Box::new(self.capabilities__protocol_connect_ws.clone())
            }
            "capabilities.protocol_accept_ws" => {
                Box::new(self.capabilities__protocol_accept_ws.clone())
            }
            "capabilities.protocol_connect_wss" => {
                Box::new(self.capabilities__protocol_connect_wss.clone())
            }
            "capabilities.protocol_accept_wss" => {
                Box::new(self.capabilities__protocol_accept_wss.clone())
            }
            "table_store.directory" => Box::new(self.table_store__directory.clone()),
            "table_store.delete" => Box::new(self.table_store__delete.clone()),
            "block_store.directory" => Box::new(self.block_store__directory.clone()),
            "block_store.delete" => Box::new(self.block_store__delete.clone()),
            "protected_store.allow_insecure_fallback" => {
                Box::new(self.protected_store__allow_insecure_fallback.clone())
            }
            "protected_store.always_use_insecure_storage" => {
                Box::new(self.protected_store__always_use_insecure_storage.clone())
            }
            "protected_store.insecure_fallback_directory" => {
                Box::new(self.protected_store__insecure_fallback_directory.clone())
            }
            "protected_store.delete" => Box::new(self.protected_store__delete.clone()),
            "network.node_id" => Box::new(self.network__node_id.clone()),
            "network.node_id_secret" => Box::new(self.network__node_id_secret.clone()),
            "network.max_connections" => Box::new(self.network__max_connections.clone()),
            "network.connection_initial_timeout_ms" => {
                Box::new(self.network__connection_initial_timeout_ms.clone())
            }
            "network.bootstrap" => Box::new(self.network__bootstrap.clone()),
            "network.dht.resolve_node_timeout_ms" => {
                Box::new(self.network__dht__resolve_node_timeout_ms.clone())
            }
            "network.dht.resolve_node_count" => {
                Box::new(self.network__dht__resolve_node_count.clone())
            }
            "network.dht.resolve_node_fanout" => {
                Box::new(self.network__dht__resolve_node_fanout.clone())
            }
            "network.dht.max_find_node_count" => {
                Box::new(self.network__dht__max_find_node_count.clone())
            }
            "network.dht.get_value_timeout_ms" => {
                Box::new(self.network__dht__get_value_timeout_ms.clone())
            }
            "network.dht.get_value_count" => Box::new(self.network__dht__get_value_count.clone()),
            "network.dht.get_value_fanout" => Box::new(self.network__dht__get_value_fanout.clone()),
            "network.dht.set_value_timeout_ms" => {
                Box::new(self.network__dht__set_value_timeout_ms.clone())
            }
            "network.dht.set_value_count" => Box::new(self.network__dht__set_value_count.clone()),
            "network.dht.set_value_fanout" => Box::new(self.network__dht__set_value_fanout.clone()),
            "network.dht.min_peer_count" => Box::new(self.network__dht__min_peer_count.clone()),
            "network.dht.min_peer_refresh_time_ms" => {
                Box::new(self.network__dht__min_peer_refresh_time_ms.clone())
            }
            "network.dht.validate_dial_info_receipt_time_ms" => Box::new(
                self.network__dht__validate_dial_info_receipt_time_ms
                    .clone(),
            ),
            "network.rpc.concurrency" => Box::new(self.network__rpc__concurrency.clone()),
            "network.rpc.queue_size" => Box::new(self.network__rpc__queue_size.clone()),
            "network.rpc.max_timestamp_behind_ms" => {
                Box::new(self.network__rpc__max_timestamp_behind_ms.clone())
            }
            "network.rpc.max_timestamp_ahead_ms" => {
                Box::new(self.network__rpc__max_timestamp_ahead_ms.clone())
            }
            "network.rpc.timeout_ms" => Box::new(self.network__rpc__timeout_ms.clone()),
            "network.rpc.max_route_hop_count" => {
                Box::new(self.network__rpc__max_route_hop_count.clone())
            }
            "network.upnp" => Box::new(self.network__upnp.clone()),
            "network.natpmp" => Box::new(self.network__natpmp.clone()),
            "network.enable_local_peer_scope" => {
                Box::new(self.network__enable_local_peer_scope.clone())
            }
            "network.restricted_nat_retries" => {
                Box::new(self.network__restricted_nat_retries.clone())
            }
            "network.tls.certificate_path" => Box::new("".to_owned()),
            "network.tls.private_key_path" => Box::new("".to_owned()),
            "network.tls.connection_initial_timeout" => Box::new(0u32),
            "network.application.https.enabled" => Box::new(false),
            "network.application.https.listen_address" => Box::new("".to_owned()),
            "network.application.https.path" => Box::new("".to_owned()),
            "network.application.https.url" => Box::new(Option::<String>::None),
            "network.application.http.enabled" => Box::new(false),
            "network.application.http.listen_address" => Box::new("".to_owned()),
            "network.application.http.path" => Box::new("".to_owned()),
            "network.application.http.url" => Box::new(Option::<String>::None),
            "network.protocol.udp.enabled" => {
                Box::new(self.network__protocol__udp__enabled.clone())
            }
            "network.protocol.udp.socket_pool_size" => {
                Box::new(self.network__protocol__udp__socket_pool_size.clone())
            }
            "network.protocol.udp.listen_address" => {
                Box::new(self.network__protocol__udp__listen_address.clone())
            }
            "network.protocol.udp.public_address" => {
                Box::new(self.network__protocol__udp__public_address.clone())
            }
            "network.protocol.tcp.connect" => {
                Box::new(self.network__protocol__tcp__connect.clone())
            }
            "network.protocol.tcp.listen" => Box::new(self.network__protocol__tcp__listen.clone()),
            "network.protocol.tcp.max_connections" => {
                Box::new(self.network__protocol__tcp__max_connections.clone())
            }
            "network.protocol.tcp.listen_address" => {
                Box::new(self.network__protocol__tcp__listen_address.clone())
            }
            "network.protocol.tcp.public_address" => {
                Box::new(self.network__protocol__tcp__public_address.clone())
            }
            "network.protocol.ws.connect" => Box::new(self.network__protocol__ws__connect.clone()),
            "network.protocol.ws.listen" => Box::new(self.network__protocol__ws__listen.clone()),
            "network.protocol.ws.max_connections" => {
                Box::new(self.network__protocol__ws__max_connections.clone())
            }
            "network.protocol.ws.listen_address" => {
                Box::new(self.network__protocol__ws__listen_address.clone())
            }
            "network.protocol.ws.path" => Box::new(self.network__protocol__ws__path.clone()),
            "network.protocol.ws.url" => Box::new(self.network__protocol__ws__url.clone()),
            "network.protocol.wss.connect" => {
                Box::new(self.network__protocol__wss__connect.clone())
            }
            "network.protocol.wss.listen" => Box::new(false),
            "network.protocol.wss.max_connections" => {
                Box::new(self.network__protocol__wss__max_connections.clone())
            }
            "network.protocol.wss.listen_address" => Box::new("".to_owned()),
            "network.protocol.wss.path" => Box::new("".to_owned()),
            "network.protocol.wss.url" => Box::new(Option::<String>::None),
            "network.leases.max_server_signal_leases" => {
                Box::new(self.network__leases__max_server_signal_leases.clone())
            }
            "network.leases.max_server_relay_leases" => {
                Box::new(self.network__leases__max_server_relay_leases.clone())
            }
            "network.leases.max_client_signal_leases" => {
                Box::new(self.network__leases__max_client_signal_leases.clone())
            }
            "network.leases.max_client_relay_leases" => {
                Box::new(self.network__leases__max_client_relay_leases.clone())
            }
            _ => {
                let err = format!("config key '{}' doesn't exist", key);
                error!("{}", err);
                return Err(err);
            }
        };
        std::result::Result::Ok(out)
    }
}

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
            veilid_core::VeilidAPIError::NodeNotFound(node_id) => {
                VeilidAPIError::NodeNotFound(format!("{}", node_id))
            }
            veilid_core::VeilidAPIError::NoDialInfo(node_id) => {
                VeilidAPIError::NodeNotFound(format!("{}", node_id))
            }
            veilid_core::VeilidAPIError::Internal(msg) => VeilidAPIError::Internal(msg.clone()),
            veilid_core::VeilidAPIError::Unimplemented(msg) => {
                VeilidAPIError::Unimplemented(msg.clone())
            }
            veilid_core::VeilidAPIError::ParseError { message, value } => {
                VeilidAPIError::ParseError {
                    message: message.clone(),
                    value: value.clone(),
                }
            }
            veilid_core::VeilidAPIError::InvalidArgument {
                context,
                argument,
                value,
            } => VeilidAPIError::InvalidArgument {
                context: context.clone(),
                argument: argument.clone(),
                value: value.clone(),
            },
            veilid_core::VeilidAPIError::MissingArgument { context, argument } => {
                VeilidAPIError::MissingArgument {
                    context: context.clone(),
                    argument: argument.clone(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
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
            veilid_core::AttachmentState::Attaching => AttachmentState::Attaching,
            veilid_core::AttachmentState::AttachedWeak => AttachmentState::AttachedWeak,
            veilid_core::AttachmentState::AttachedGood => AttachmentState::AttachedGood,
            veilid_core::AttachmentState::AttachedStrong => AttachmentState::AttachedStrong,
            veilid_core::AttachmentState::FullyAttached => AttachmentState::FullyAttached,
            veilid_core::AttachmentState::OverAttached => AttachmentState::OverAttached,
            veilid_core::AttachmentState::Detaching => AttachmentState::Detaching,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VeilidUpdate {
    Attachment(AttachmentState),
}

impl VeilidUpdate {
    fn from_core(veilid_update: veilid_core::VeilidUpdate) -> Self {
        match veilid_update {
            veilid_core::VeilidUpdate::Attachment(attachment) => {
                Self::Attachment(AttachmentState::from_core(attachment))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct VeilidState {
    pub attachment: AttachmentState,
}

impl VeilidState {
    fn from_core(veilid_state: veilid_core::VeilidState) -> Self {
        Self {
            attachment: AttachmentState::from_core(veilid_state.attachment),
        }
    }
}

/////////////////////////////////////////
pub fn startup_veilid_core(
    sink: StreamSink<VeilidUpdate>,
    config: VeilidConfig,
) -> Result<VeilidState> {
    async_std::task::block_on(async {
        let mut api_lock = VEILID_API.lock().await;
        if api_lock.is_some() {
            return Err(anyhow!(VeilidAPIError::AlreadyInitialized));
        }

        let core = veilid_core::VeilidCore::new();

        let setup = veilid_core::VeilidCoreSetup {
            update_callback: Arc::new(
                move |update: veilid_core::VeilidUpdate| -> veilid_core::SystemPinBoxFuture<()> {
                    let sink = sink.clone();
                    Box::pin(async move {
                        if !sink.add(VeilidUpdate::from_core(update)) {
                            error!("error sending veilid update callback");
                        }
                    })
                },
            ),
            config_callback: Arc::new(move |key| config.get_by_str(&key)),
        };

        let veilid_api = core
            .startup(setup)
            .await
            .map_err(|e| VeilidAPIError::InvalidConfig(e.clone()))?;
        *api_lock = Some(veilid_api.clone());

        let core_state = veilid_api
            .get_state()
            .await
            .map_err(VeilidAPIError::from_core)?;
        Ok(VeilidState::from_core(core_state))
    })
}

pub fn get_veilid_state() -> Result<VeilidState> {
    async_std::task::block_on(async {
        let veilid_api = get_veilid_api().await?;
        let core_state = veilid_api
            .get_state()
            .await
            .map_err(VeilidAPIError::from_core)?;
        Ok(VeilidState::from_core(core_state))
    })
}

// xxx api functions

pub fn shutdown_veilid_core() -> Result<()> {
    async_std::task::block_on(async {
        let veilid_api = get_veilid_api().await?;
        veilid_api.shutdown().await;
        Ok(())
    })
}

pub fn veilid_version_string() -> Result<String> {
    Ok(veilid_core::veilid_version_string())
}

pub struct VeilidVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

pub fn veilid_version() -> Result<VeilidVersion> {
    let (major, minor, patch) = veilid_core::veilid_version();
    Ok(VeilidVersion {
        major,
        minor,
        patch,
    })
}

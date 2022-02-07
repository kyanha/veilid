use cfg_if::*;
use log::*;
use serde::*;

/////////////////////////////////////////
// Config Settings
// Not all settings available through Veilid API are available to Flutter applications

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct VeilidConfig {
    pub program_name: String,
    pub veilid_namespace: String,
    pub api_log_level: veilid_core::VeilidConfigLogLevel,
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

cfg_if! {
    if #[cfg(target_arch="wasm32")] {
        type ConfigReturn = Box<dyn std::any::Any + 'static>;
    } else {
        type ConfigReturn = Box<dyn std::any::Any + Send + 'static>;
    }
}

impl VeilidConfig {
    pub fn get_by_str(&self, key: &str) -> std::result::Result<ConfigReturn, String> {
        let out: ConfigReturn = match key {
            "program_name" => Box::new(self.program_name.clone()),
            "namespace" => Box::new(self.veilid_namespace.clone()),
            "api_log_level" => Box::new(self.api_log_level),
            "capabilities.protocol_udp" => Box::new(self.capabilities__protocol_udp),
            "capabilities.protocol_connect_tcp" => {
                Box::new(self.capabilities__protocol_connect_tcp)
            }
            "capabilities.protocol_accept_tcp" => Box::new(self.capabilities__protocol_accept_tcp),
            "capabilities.protocol_connect_ws" => Box::new(self.capabilities__protocol_connect_ws),
            "capabilities.protocol_accept_ws" => Box::new(self.capabilities__protocol_accept_ws),
            "capabilities.protocol_connect_wss" => {
                Box::new(self.capabilities__protocol_connect_wss)
            }
            "capabilities.protocol_accept_wss" => Box::new(self.capabilities__protocol_accept_wss),
            "table_store.directory" => Box::new(self.table_store__directory.clone()),
            "table_store.delete" => Box::new(self.table_store__delete),
            "block_store.directory" => Box::new(self.block_store__directory.clone()),
            "block_store.delete" => Box::new(self.block_store__delete),
            "protected_store.allow_insecure_fallback" => {
                Box::new(self.protected_store__allow_insecure_fallback)
            }
            "protected_store.always_use_insecure_storage" => {
                Box::new(self.protected_store__always_use_insecure_storage)
            }
            "protected_store.insecure_fallback_directory" => {
                Box::new(self.protected_store__insecure_fallback_directory.clone())
            }
            "protected_store.delete" => Box::new(self.protected_store__delete),
            "network.node_id" => Box::new(self.network__node_id.clone()),
            "network.node_id_secret" => Box::new(self.network__node_id_secret.clone()),
            "network.max_connections" => Box::new(self.network__max_connections),
            "network.connection_initial_timeout_ms" => {
                Box::new(self.network__connection_initial_timeout_ms)
            }
            "network.bootstrap" => Box::new(self.network__bootstrap.clone()),
            "network.dht.resolve_node_timeout_ms" => {
                Box::new(self.network__dht__resolve_node_timeout_ms)
            }
            "network.dht.resolve_node_count" => Box::new(self.network__dht__resolve_node_count),
            "network.dht.resolve_node_fanout" => Box::new(self.network__dht__resolve_node_fanout),
            "network.dht.max_find_node_count" => Box::new(self.network__dht__max_find_node_count),
            "network.dht.get_value_timeout_ms" => Box::new(self.network__dht__get_value_timeout_ms),
            "network.dht.get_value_count" => Box::new(self.network__dht__get_value_count),
            "network.dht.get_value_fanout" => Box::new(self.network__dht__get_value_fanout),
            "network.dht.set_value_timeout_ms" => Box::new(self.network__dht__set_value_timeout_ms),
            "network.dht.set_value_count" => Box::new(self.network__dht__set_value_count),
            "network.dht.set_value_fanout" => Box::new(self.network__dht__set_value_fanout),
            "network.dht.min_peer_count" => Box::new(self.network__dht__min_peer_count),
            "network.dht.min_peer_refresh_time_ms" => {
                Box::new(self.network__dht__min_peer_refresh_time_ms)
            }
            "network.dht.validate_dial_info_receipt_time_ms" => {
                Box::new(self.network__dht__validate_dial_info_receipt_time_ms)
            }
            "network.rpc.concurrency" => Box::new(self.network__rpc__concurrency),
            "network.rpc.queue_size" => Box::new(self.network__rpc__queue_size),
            "network.rpc.max_timestamp_behind_ms" => {
                Box::new(self.network__rpc__max_timestamp_behind_ms)
            }
            "network.rpc.max_timestamp_ahead_ms" => {
                Box::new(self.network__rpc__max_timestamp_ahead_ms)
            }
            "network.rpc.timeout_ms" => Box::new(self.network__rpc__timeout_ms),
            "network.rpc.max_route_hop_count" => Box::new(self.network__rpc__max_route_hop_count),
            "network.upnp" => Box::new(self.network__upnp),
            "network.natpmp" => Box::new(self.network__natpmp),
            "network.enable_local_peer_scope" => Box::new(self.network__enable_local_peer_scope),
            "network.restricted_nat_retries" => Box::new(self.network__restricted_nat_retries),
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
            "network.protocol.udp.enabled" => Box::new(self.network__protocol__udp__enabled),
            "network.protocol.udp.socket_pool_size" => {
                Box::new(self.network__protocol__udp__socket_pool_size)
            }
            "network.protocol.udp.listen_address" => {
                Box::new(self.network__protocol__udp__listen_address.clone())
            }
            "network.protocol.udp.public_address" => {
                Box::new(self.network__protocol__udp__public_address.clone())
            }
            "network.protocol.tcp.connect" => Box::new(self.network__protocol__tcp__connect),
            "network.protocol.tcp.listen" => Box::new(self.network__protocol__tcp__listen),
            "network.protocol.tcp.max_connections" => {
                Box::new(self.network__protocol__tcp__max_connections)
            }
            "network.protocol.tcp.listen_address" => {
                Box::new(self.network__protocol__tcp__listen_address.clone())
            }
            "network.protocol.tcp.public_address" => {
                Box::new(self.network__protocol__tcp__public_address.clone())
            }
            "network.protocol.ws.connect" => Box::new(self.network__protocol__ws__connect),
            "network.protocol.ws.listen" => Box::new(self.network__protocol__ws__listen),
            "network.protocol.ws.max_connections" => {
                Box::new(self.network__protocol__ws__max_connections)
            }
            "network.protocol.ws.listen_address" => {
                Box::new(self.network__protocol__ws__listen_address.clone())
            }
            "network.protocol.ws.path" => Box::new(self.network__protocol__ws__path.clone()),
            "network.protocol.ws.url" => Box::new(self.network__protocol__ws__url.clone()),
            "network.protocol.wss.connect" => Box::new(self.network__protocol__wss__connect),
            "network.protocol.wss.listen" => Box::new(false),
            "network.protocol.wss.max_connections" => {
                Box::new(self.network__protocol__wss__max_connections)
            }
            "network.protocol.wss.listen_address" => Box::new("".to_owned()),
            "network.protocol.wss.path" => Box::new("".to_owned()),
            "network.protocol.wss.url" => Box::new(Option::<String>::None),
            "network.leases.max_server_signal_leases" => {
                Box::new(self.network__leases__max_server_signal_leases)
            }
            "network.leases.max_server_relay_leases" => {
                Box::new(self.network__leases__max_server_relay_leases)
            }
            "network.leases.max_client_signal_leases" => {
                Box::new(self.network__leases__max_client_signal_leases)
            }
            "network.leases.max_client_relay_leases" => {
                Box::new(self.network__leases__max_client_relay_leases)
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

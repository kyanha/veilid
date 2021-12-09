use crate::dht::key;
use crate::intf;
use crate::xx::*;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub type ConfigCallback = Arc<dyn Fn(String) -> Result<Box<dyn core::any::Any>, String>>;
    } else {
        pub type ConfigCallback = Arc<dyn Fn(String) -> Result<Box<dyn core::any::Any>, String> + Send>;
    }
}

#[derive(Default, Clone)]
pub struct VeilidConfigHTTPS {
    pub enabled: bool,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>, // Fixed URL is not optional for TLS-based protocols and is dynamically validated
}

#[derive(Default, Clone)]
pub struct VeilidConfigHTTP {
    pub enabled: bool,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>,
}

#[derive(Default, Clone)]
pub struct VeilidConfigApplication {
    pub https: VeilidConfigHTTPS,
    pub http: VeilidConfigHTTP,
}

#[derive(Default, Clone)]
pub struct VeilidConfigUDP {
    pub enabled: bool,
    pub socket_pool_size: u32,
    pub listen_address: String,
    pub public_address: Option<String>,
}

#[derive(Default, Clone)]
pub struct VeilidConfigTCP {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub public_address: Option<String>,
}

#[derive(Default, Clone)]
pub struct VeilidConfigWS {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>,
}

#[derive(Default, Clone)]
pub struct VeilidConfigWSS {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>, // Fixed URL is not optional for TLS-based protocols and is dynamically validated
}

#[derive(Default, Clone)]
pub struct VeilidConfigProtocol {
    pub udp: VeilidConfigUDP,
    pub tcp: VeilidConfigTCP,
    pub ws: VeilidConfigWS,
    pub wss: VeilidConfigWSS,
}

#[derive(Default, Clone)]
pub struct VeilidConfigTLS {
    pub certificate_path: String,
    pub private_key_path: String,
    pub connection_initial_timeout: u64,
}

#[derive(Default, Clone)]
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

#[derive(Default, Clone)]
pub struct VeilidConfigRPC {
    pub concurrency: u32,
    pub queue_size: u32,
    pub max_timestamp_behind: Option<u64>,
    pub max_timestamp_ahead: Option<u64>,
    pub timeout: u64,
    pub max_route_hop_count: u8,
}

#[derive(Default, Clone)]
pub struct VeilidConfigLeases {
    pub max_server_signal_leases: u32,
    pub max_server_relay_leases: u32,
    pub max_client_signal_leases: u32,
    pub max_client_relay_leases: u32,
}

#[derive(Default, Clone)]
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
    pub address_filter: bool,
    pub restricted_nat_retries: u32,
    pub tls: VeilidConfigTLS,
    pub application: VeilidConfigApplication,
    pub protocol: VeilidConfigProtocol,
    pub leases: VeilidConfigLeases,
}

#[derive(Default, Clone)]
pub struct VeilidConfigTableStore {
    pub directory: String,
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
pub struct VeilidConfigInner {
    pub namespace: String,
    pub capabilities: VeilidConfigCapabilities,
    pub tablestore: VeilidConfigTableStore,
    pub network: VeilidConfigNetwork,
}

#[derive(Clone)]
pub struct VeilidConfig {
    inner: Arc<RwLock<VeilidConfigInner>>,
}

impl Default for VeilidConfig {
    fn default() -> Self {
        Self::new()
    }
}
impl VeilidConfig {
    fn new_inner() -> VeilidConfigInner {
        VeilidConfigInner::default()
    }

    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Self::new_inner())),
        }
    }

    pub async fn init(&mut self, cb: ConfigCallback) -> Result<(), String> {
        macro_rules! get_config {
            ($key:expr) => {
                let keyname = &stringify!($key)[6..];
                $key = *cb(keyname.to_owned())?.downcast().map_err(|_| {
                    let err = format!("incorrect type for key: {}", keyname);
                    debug!("{}", err);
                    err
                })?;
            };
        }

        {
            let mut inner = self.inner.write();
            get_config!(inner.namespace);
            get_config!(inner.capabilities.protocol_udp);
            get_config!(inner.capabilities.protocol_connect_tcp);
            get_config!(inner.capabilities.protocol_accept_tcp);
            get_config!(inner.capabilities.protocol_connect_ws);
            get_config!(inner.capabilities.protocol_accept_ws);
            get_config!(inner.capabilities.protocol_connect_wss);
            get_config!(inner.capabilities.protocol_accept_wss);
            get_config!(inner.tablestore.directory);
            get_config!(inner.network.node_id);
            get_config!(inner.network.node_id_secret);
            get_config!(inner.network.max_connections);
            get_config!(inner.network.connection_initial_timeout);
            get_config!(inner.network.bootstrap);
            get_config!(inner.network.dht.resolve_node_timeout);
            get_config!(inner.network.dht.resolve_node_count);
            get_config!(inner.network.dht.resolve_node_fanout);
            get_config!(inner.network.dht.max_find_node_count);
            get_config!(inner.network.dht.get_value_timeout);
            get_config!(inner.network.dht.get_value_count);
            get_config!(inner.network.dht.get_value_fanout);
            get_config!(inner.network.dht.set_value_timeout);
            get_config!(inner.network.dht.set_value_count);
            get_config!(inner.network.dht.set_value_fanout);
            get_config!(inner.network.dht.min_peer_count);
            get_config!(inner.network.dht.min_peer_refresh_time);
            get_config!(inner.network.dht.validate_dial_info_receipt_time);
            get_config!(inner.network.rpc.concurrency);
            get_config!(inner.network.rpc.queue_size);
            get_config!(inner.network.rpc.max_timestamp_behind);
            get_config!(inner.network.rpc.max_timestamp_ahead);
            get_config!(inner.network.rpc.timeout);
            get_config!(inner.network.rpc.max_route_hop_count);
            get_config!(inner.network.upnp);
            get_config!(inner.network.natpmp);
            get_config!(inner.network.address_filter);
            get_config!(inner.network.restricted_nat_retries);
            get_config!(inner.network.tls.certificate_path);
            get_config!(inner.network.tls.private_key_path);
            get_config!(inner.network.tls.connection_initial_timeout);
            get_config!(inner.network.application.https.enabled);
            get_config!(inner.network.application.https.listen_address);
            get_config!(inner.network.application.https.path);
            get_config!(inner.network.application.https.url);
            get_config!(inner.network.application.http.enabled);
            get_config!(inner.network.application.http.listen_address);
            get_config!(inner.network.application.http.path);
            get_config!(inner.network.application.http.url);
            get_config!(inner.network.protocol.udp.enabled);
            get_config!(inner.network.protocol.udp.socket_pool_size);
            get_config!(inner.network.protocol.udp.listen_address);
            get_config!(inner.network.protocol.udp.public_address);
            get_config!(inner.network.protocol.tcp.connect);
            get_config!(inner.network.protocol.tcp.listen);
            get_config!(inner.network.protocol.tcp.max_connections);
            get_config!(inner.network.protocol.tcp.listen_address);
            get_config!(inner.network.protocol.tcp.public_address);
            get_config!(inner.network.protocol.ws.connect);
            get_config!(inner.network.protocol.ws.listen);
            get_config!(inner.network.protocol.ws.max_connections);
            get_config!(inner.network.protocol.ws.listen_address);
            get_config!(inner.network.protocol.ws.path);
            get_config!(inner.network.protocol.ws.url);
            get_config!(inner.network.protocol.wss.connect);
            get_config!(inner.network.protocol.wss.listen);
            get_config!(inner.network.protocol.wss.max_connections);
            get_config!(inner.network.protocol.wss.listen_address);
            get_config!(inner.network.protocol.wss.path);
            get_config!(inner.network.protocol.wss.url);
            get_config!(inner.network.leases.max_server_signal_leases);
            get_config!(inner.network.leases.max_server_relay_leases);
            get_config!(inner.network.leases.max_client_signal_leases);
            get_config!(inner.network.leases.max_client_relay_leases);
        }

        // Initialize node id as early as possible because it is used
        // for encryption purposes all over the program
        self.init_node_id().await?;

        // Validate settings
        self.validate().await?;

        Ok(())
    }

    pub async fn terminate(&self) {
        //
    }

    pub fn get(&self) -> RwLockReadGuard<VeilidConfigInner> {
        self.inner.read()
    }

    async fn validate(&self) -> Result<(), String> {
        let inner = self.inner.read();
        // if inner.network.protocol.udp.enabled {
        //     // Validate UDP settings
        // }
        if inner.network.protocol.tcp.listen {
            // Validate TCP settings
            if inner.network.protocol.tcp.max_connections == 0 {
                return Err("TCP max connections must be > 0 in config key 'network.protocol.tcp.max_connections'".to_owned());
            }
        }
        if inner.network.protocol.ws.listen {
            // Validate WS settings
            if inner.network.protocol.ws.max_connections == 0 {
                return Err("WS max connections must be > 0 in config key 'network.protocol.ws.max_connections'".to_owned());
            }
            if inner.network.application.https.enabled
                && inner.network.application.https.path == inner.network.protocol.ws.path
            {
                return Err("WS path conflicts with HTTPS application path in config key 'network.protocol.ws.path'".to_owned());
            }
            if inner.network.application.http.enabled
                && inner.network.application.http.path == inner.network.protocol.ws.path
            {
                return Err("WS path conflicts with HTTP application path in config key 'network.protocol.ws.path'".to_owned());
            }
        }
        if inner.network.protocol.wss.listen {
            // Validate WSS settings
            if inner.network.protocol.wss.max_connections == 0 {
                return Err("WSS max connections must be > 0 in config key 'network.protocol.wss.max_connections'".to_owned());
            }
            if inner
                .network
                .protocol
                .wss
                .url
                .as_ref()
                .map(|u| u.is_empty())
                .unwrap_or_default()
            {
                return Err(
                    "WSS URL must be specified in config key 'network.protocol.wss.url'".to_owned(),
                );
            }
            if inner.network.application.https.enabled
                && inner.network.application.https.path == inner.network.protocol.wss.path
            {
                return Err("WSS path conflicts with HTTPS application path in config key 'network.protocol.ws.path'".to_owned());
            }
            if inner.network.application.http.enabled
                && inner.network.application.http.path == inner.network.protocol.wss.path
            {
                return Err("WSS path conflicts with HTTP application path in config key 'network.protocol.ws.path'".to_owned());
            }
        }
        if inner.network.application.https.enabled {
            // Validate HTTPS settings
            if inner
                .network
                .application
                .https
                .url
                .as_ref()
                .map(|u| u.is_empty())
                .unwrap_or_default()
            {
                return Err(
                    "HTTPS URL must be specified in config key 'network.application.https.url'"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }

    // Get the node id from config if one is specified
    async fn init_node_id(&self) -> Result<(), String> {
        let mut inner = self.inner.write();

        let namespace = inner.namespace.clone();
        let mut node_id = inner.network.node_id;
        let mut node_id_secret = inner.network.node_id_secret;
        // See if node id was previously stored in the protected store
        if !node_id.valid {
            debug!("pulling node id from storage");
            if let Some(s) = intf::load_user_secret_string(namespace.as_str(), "node_id").await? {
                debug!("node id found in storage");
                node_id = key::DHTKey::try_decode(s.as_str())?
            } else {
                debug!("node id not found in storage");
            }
        }

        // See if node id secret was previously stored in the protected store
        if !node_id_secret.valid {
            debug!("pulling node id secret from storage");
            if let Some(s) =
                intf::load_user_secret_string(namespace.as_str(), "node_id_secret").await?
            {
                debug!("node id secret found in storage");
                node_id_secret = key::DHTKeySecret::try_decode(s.as_str())?
            } else {
                debug!("node id secret not found in storage");
            }
        }

        // If we have a node id from storage, check it
        if node_id.valid && node_id_secret.valid {
            // Validate node id
            if !key::validate_key(&node_id, &node_id_secret) {
                return Err("node id secret and node id key don't match".to_owned());
            }
        }

        // If we still don't have a valid node id, generate one
        if !node_id.valid || !node_id_secret.valid {
            debug!("generating new node id");
            let (i, s) = key::generate_secret();
            node_id = i;
            node_id_secret = s;
        }
        info!("Node Id is {}", node_id.encode());
        // info!("Node Id Secret is {}", node_id_secret.encode());

        // Save the node id / secret in storage
        intf::save_user_secret_string(namespace.as_str(), "node_id", node_id.encode().as_str())
            .await?;
        intf::save_user_secret_string(
            namespace.as_str(),
            "node_id_secret",
            node_id_secret.encode().as_str(),
        )
        .await?;

        inner.network.node_id = node_id;
        inner.network.node_id_secret = node_id_secret;

        trace!("init_node_id complete");

        Ok(())
    }
}

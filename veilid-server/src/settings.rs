#![allow(clippy::bool_assert_comparison)]

use directories::*;
use log::*;
use parking_lot::*;

use serde_derive::*;
use std::ffi::OsStr;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use url::Url;

pub fn load_default_config(cfg: &mut config::Config) -> Result<(), config::ConfigError> {
    let default_config = String::from(
        r#"---
daemon: false
client_api:
    enabled: true
    listen_address: 'localhost:5959'
auto_attach: true
logging: 
    terminal:
        enabled: true
        level: 'info'
    file: 
        enabled: false
        path: ''
        append: true
        level: 'info'
    client:
        enabled: true
        level: 'info'
testing:
    subnode_index: 0
core:
    protected_store:
        allow_insecure_fallback: true
        always_use_insecure_storage: false
        insecure_fallback_directory: '%INSECURE_FALLBACK_DIRECTORY%'
        delete: false
    table_store:
        directory: '%TABLE_STORE_DIRECTORY%'
        delete: false
    block_store:
        directory: '%BLOCK_STORE_DIRECTORY%'
        delete: false
    network:
        max_connections: 16
        connection_initial_timeout: 2000000
        node_id: ''
        node_id_secret: ''
        bootstrap: []
        rpc: 
            concurrency: 0
            queue_size: 1024
            max_timestamp_behind: 10000000
            max_timestamp_ahead: 10000000
            timeout: 10000000
            max_route_hop_count: 7
        dht:
            resolve_node_timeout:
            resolve_node_count: 20
            resolve_node_fanout: 3
            max_find_node_count: 20
            get_value_timeout:
            get_value_count: 20
            get_value_fanout: 3
            set_value_timeout:
            set_value_count: 20
            set_value_fanout: 5
            min_peer_count: 20
            min_peer_refresh_time: 2000000
            validate_dial_info_receipt_time: 5000000
        upnp: false
        natpmp: false
        enable_local_peer_scope: false
        restricted_nat_retries: 3
        tls:
            certificate_path: '/etc/veilid/server.crt'
            private_key_path: '/etc/veilid/private/server.key'
            connection_initial_timeout: 2000000
        application:
            https:
                enabled: false
                listen_address: '[::]:5150'
                path: 'app'
                # url: 'https://localhost:5150'
            http:
                enabled: false
                listen_address: '[::]:5150'
                path: 'app'
                # url: 'http://localhost:5150'
        protocol:
            udp:
                enabled: true
                socket_pool_size: 0
                listen_address: '[::]:5150'
                # public_address: ''
            tcp:
                connect: true
                listen: true
                max_connections: 32
                listen_address: '[::]:5150'
                #'public_address: ''
            ws:
                connect: true
                listen: true
                max_connections: 16
                listen_address: '[::]:5150'
                path: 'ws'
                # url: 'ws://localhost:5150/ws'
            wss:
                connect: true
                listen: false
                max_connections: 16
                listen_address: '[::]:5150'
                path: 'ws'
                # url: ''
        leases:
            max_server_signal_leases: 256
            max_server_relay_leases: 8
            max_client_signal_leases: 2
            max_client_relay_leases: 2
        "#,
    )
    .replace(
        "%TABLE_STORE_DIRECTORY%",
        &Settings::get_default_table_store_path().to_string_lossy(),
    )
    .replace(
        "%BLOCK_STORE_DIRECTORY%",
        &Settings::get_default_block_store_path().to_string_lossy(),
    )
    .replace(
        "%INSECURE_FALLBACK_DIRECTORY%",
        &Settings::get_default_protected_store_insecure_fallback_directory().to_string_lossy(),
    );
    cfg.merge(config::File::from_str(
        &default_config,
        config::FileFormat::Yaml,
    ))
    .map(drop)
}

pub fn load_config(
    cfg: &mut config::Config,
    config_file: &Path,
) -> Result<(), config::ConfigError> {
    if let Some(config_file_str) = config_file.to_str() {
        cfg.merge(config::File::new(config_file_str, config::FileFormat::Yaml))
            .map(drop)
    } else {
        Err(config::ConfigError::Message(
            "config file path is not valid UTF-8".to_owned(),
        ))
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
impl<'de> serde::Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_ascii_lowercase().as_str() {
            "error" => Ok(LogLevel::Error),
            "warn" => Ok(LogLevel::Warn),
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "trace" => Ok(LogLevel::Trace),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid log level: {}",
                s
            ))),
        }
    }
}
impl serde::Serialize for LogLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        };
        s.serialize(serializer)
    }
}

pub fn convert_loglevel(log_level: LogLevel) -> LevelFilter {
    match log_level {
        LogLevel::Error => LevelFilter::Error,
        LogLevel::Warn => LevelFilter::Warn,
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Debug => LevelFilter::Debug,
        LogLevel::Trace => LevelFilter::Trace,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedUrl {
    pub urlstring: String,
    pub url: Url,
}

impl ParsedUrl {
    pub fn offset_port(&mut self, offset: u16) -> Result<(), ()> {
        // Bump port on url
        self.url.set_port(Some(self.url.port().unwrap() + offset))?;
        self.urlstring = self.url.to_string();
        Ok(())
    }
}

impl FromStr for ParsedUrl {
    type Err = url::ParseError;
    fn from_str(s: &str) -> Result<ParsedUrl, url::ParseError> {
        let mut url = Url::parse(s)?;
        if url.scheme().to_lowercase() == "http" && url.port().is_none() {
            url.set_port(Some(80))
                .map_err(|_| url::ParseError::InvalidPort)?
        }
        if url.scheme().to_lowercase() == "https" && url.port().is_none() {
            url.set_port(Some(443))
                .map_err(|_| url::ParseError::InvalidPort)?;
        }
        let parsed_urlstring = url.to_string();
        Ok(Self {
            urlstring: parsed_urlstring,
            url,
        })
    }
}

impl<'de> serde::Deserialize<'de> for ParsedUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ParsedUrl::from_str(s.as_str()).map_err(serde::de::Error::custom)
    }
}

impl serde::Serialize for ParsedUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.urlstring.serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNodeDialInfo {
    pub node_dial_info_string: String,
    pub node_dial_info: veilid_core::NodeDialInfo,
}

// impl ParsedNodeDialInfo {
//     pub fn offset_port(&mut self, offset: u16) -> Result<(), ()> {
//         // Bump port on dial_info
//         self.node_dial_info
//             .dial_info
//             .set_port(self.node_dial_info.dial_info.port() + 1);
//         self.node_dial_info_string = self.node_dial_info.to_string();
//         Ok(())
//     }
// }

impl FromStr for ParsedNodeDialInfo {
    type Err = veilid_core::VeilidAPIError;
    fn from_str(
        node_dial_info_string: &str,
    ) -> Result<ParsedNodeDialInfo, veilid_core::VeilidAPIError> {
        let node_dial_info = veilid_core::NodeDialInfo::from_str(node_dial_info_string)?;
        Ok(Self {
            node_dial_info_string: node_dial_info_string.to_owned(),
            node_dial_info,
        })
    }
}

impl<'de> serde::Deserialize<'de> for ParsedNodeDialInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ParsedNodeDialInfo::from_str(s.as_str()).map_err(serde::de::Error::custom)
    }
}

impl serde::Serialize for ParsedNodeDialInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.node_dial_info_string.serialize(serializer)
    }
}

#[derive(Debug, PartialEq)]
pub struct NamedSocketAddrs {
    pub name: String,
    pub addrs: Vec<SocketAddr>,
}

impl FromStr for NamedSocketAddrs {
    type Err = std::io::Error;
    fn from_str(s: &str) -> Result<NamedSocketAddrs, std::io::Error> {
        let addr_iter = s.to_socket_addrs()?;
        Ok(NamedSocketAddrs {
            name: s.to_owned(),
            addrs: addr_iter.collect(),
        })
    }
}

impl<'de> serde::Deserialize<'de> for NamedSocketAddrs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NamedSocketAddrs::from_str(s.as_str()).map_err(serde::de::Error::custom)
    }
}

impl serde::Serialize for NamedSocketAddrs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.name.serialize(serializer)
    }
}

impl NamedSocketAddrs {
    pub fn offset_port(&mut self, offset: u16) -> Result<(), ()> {
        // Bump port on name
        if let Some(split) = self.name.rfind(':') {
            let hoststr = &self.name[0..split];
            let portstr = &self.name[split + 1..];
            let port: u16 = portstr.parse::<u16>().map_err(drop)? + offset;

            self.name = format!("{}:{}", hoststr, port.to_string());
        } else {
            return Err(());
        }

        // Bump port on addresses
        for addr in self.addrs.iter_mut() {
            addr.set_port(addr.port() + offset);
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Terminal {
    pub enabled: bool,
    pub level: LogLevel,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct File {
    pub enabled: bool,
    pub path: String,
    pub append: bool,
    pub level: LogLevel,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Client {
    pub enabled: bool,
    pub level: LogLevel,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientApi {
    pub enabled: bool,
    pub listen_address: NamedSocketAddrs,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Logging {
    pub terminal: Terminal,
    pub file: File,
    pub client: Client,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Https {
    pub enabled: bool,
    pub listen_address: NamedSocketAddrs,
    pub path: PathBuf,
    pub url: Option<ParsedUrl>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Http {
    pub enabled: bool,
    pub listen_address: NamedSocketAddrs,
    pub path: PathBuf,
    pub url: Option<ParsedUrl>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Application {
    pub https: Https,
    pub http: Http,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Udp {
    pub enabled: bool,
    pub socket_pool_size: u32,
    pub listen_address: NamedSocketAddrs,
    pub public_address: Option<NamedSocketAddrs>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tcp {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: NamedSocketAddrs,
    pub public_address: Option<NamedSocketAddrs>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Ws {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: NamedSocketAddrs,
    pub path: PathBuf,
    pub url: Option<ParsedUrl>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Wss {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: NamedSocketAddrs,
    pub path: PathBuf,
    pub url: Option<ParsedUrl>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Protocol {
    pub udp: Udp,
    pub tcp: Tcp,
    pub ws: Ws,
    pub wss: Wss,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tls {
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
    pub connection_initial_timeout: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Rpc {
    pub concurrency: u32,
    pub queue_size: u32,
    pub max_timestamp_behind: Option<u64>,
    pub max_timestamp_ahead: Option<u64>,
    pub timeout: u64,
    pub max_route_hop_count: u8,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dht {
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Leases {
    pub max_server_signal_leases: u32,
    pub max_server_relay_leases: u32,
    pub max_client_signal_leases: u32,
    pub max_client_relay_leases: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Network {
    pub max_connections: u32,
    pub connection_initial_timeout: u64,
    pub node_id: veilid_core::DHTKey,
    pub node_id_secret: veilid_core::DHTKeySecret,
    pub bootstrap: Vec<ParsedNodeDialInfo>,
    pub rpc: Rpc,
    pub dht: Dht,
    pub upnp: bool,
    pub natpmp: bool,
    pub enable_local_peer_scope: bool,
    pub restricted_nat_retries: u32,
    pub tls: Tls,
    pub application: Application,
    pub protocol: Protocol,
    pub leases: Leases,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Testing {
    pub subnode_index: u16,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TableStore {
    pub directory: PathBuf,
    pub delete: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BlockStore {
    pub directory: PathBuf,
    pub delete: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProtectedStore {
    pub allow_insecure_fallback: bool,
    pub always_use_insecure_storage: bool,
    pub insecure_fallback_directory: PathBuf,
    pub delete: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Core {
    pub protected_store: ProtectedStore,
    pub table_store: TableStore,
    pub block_store: BlockStore,
    pub network: Network,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SettingsInner {
    pub daemon: bool,
    pub client_api: ClientApi,
    pub auto_attach: bool,
    pub logging: Logging,
    pub testing: Testing,
    pub core: Core,
}

#[derive(Clone, Debug)]
pub struct Settings {
    inner: Arc<RwLock<SettingsInner>>,
}

impl Settings {
    pub fn new(
        config_file_is_default: bool,
        config_file: &OsStr,
    ) -> Result<Self, config::ConfigError> {
        // Create a config
        let mut cfg = config::Config::default();

        // Load the default config
        load_default_config(&mut cfg)?;

        // Merge in the config file if we have one
        let config_file_path = Path::new(config_file);
        if !config_file_is_default || config_file_path.exists() {
            // If the user specifies a config file on the command line then it must exist
            load_config(&mut cfg, config_file_path)?;
        }

        // Generate config
        let inner: SettingsInner = cfg.try_into()?;

        //
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }
    pub fn read(&self) -> RwLockReadGuard<SettingsInner> {
        self.inner.read()
    }
    pub fn write(&self) -> RwLockWriteGuard<SettingsInner> {
        self.inner.write()
    }

    pub fn apply_subnode_index(&self) -> Result<(), ()> {
        let mut settingsrw = self.write();
        let idx = settingsrw.testing.subnode_index;
        if idx == 0 {
            return Ok(());
        }

        // bump client api port
        (*settingsrw).client_api.listen_address.offset_port(idx)?;

        // bump protocol ports
        (*settingsrw)
            .core
            .network
            .protocol
            .udp
            .listen_address
            .offset_port(idx)?;
        (*settingsrw)
            .core
            .network
            .protocol
            .tcp
            .listen_address
            .offset_port(idx)?;
        (*settingsrw)
            .core
            .network
            .protocol
            .ws
            .listen_address
            .offset_port(idx)?;
        if let Some(url) = &mut (*settingsrw).core.network.protocol.ws.url {
            url.offset_port(idx)?;
        }
        (*settingsrw)
            .core
            .network
            .protocol
            .wss
            .listen_address
            .offset_port(idx)?;
        if let Some(url) = &mut (*settingsrw).core.network.protocol.wss.url {
            url.offset_port(idx)?;
        }
        // bump application ports
        (*settingsrw)
            .core
            .network
            .application
            .http
            .listen_address
            .offset_port(idx)?;
        if let Some(url) = &mut (*settingsrw).core.network.application.http.url {
            url.offset_port(idx)?;
        }
        (*settingsrw)
            .core
            .network
            .application
            .https
            .listen_address
            .offset_port(idx)?;
        if let Some(url) = &mut (*settingsrw).core.network.application.https.url {
            url.offset_port(idx)?;
        }
        Ok(())
    }

    pub fn get_default_config_path() -> PathBuf {
        // Get default configuration file location
        let mut default_config_path;

        if let Some(my_proj_dirs) = ProjectDirs::from("org", "Veilid", "Veilid") {
            default_config_path = PathBuf::from(my_proj_dirs.config_dir());
        } else {
            default_config_path = PathBuf::from("./");
        }
        default_config_path.push("veilid-server.conf");

        default_config_path
    }

    pub fn get_default_table_store_path() -> PathBuf {
        // Get default configuration file location
        let mut default_config_path;

        if let Some(my_proj_dirs) = ProjectDirs::from("org", "Veilid", "Veilid") {
            default_config_path = PathBuf::from(my_proj_dirs.data_local_dir());
        } else {
            default_config_path = PathBuf::from("./");
        }
        default_config_path.push("table_store");

        default_config_path
    }

    pub fn get_default_block_store_path() -> PathBuf {
        // Get default configuration file location
        let mut default_config_path;

        if let Some(my_proj_dirs) = ProjectDirs::from("org", "Veilid", "Veilid") {
            default_config_path = PathBuf::from(my_proj_dirs.data_local_dir());
        } else {
            default_config_path = PathBuf::from("./");
        }
        default_config_path.push("block_store");

        default_config_path
    }

    pub fn get_default_protected_store_insecure_fallback_directory() -> PathBuf {
        // Get default configuration file location
        let mut default_config_path;

        if let Some(my_proj_dirs) = ProjectDirs::from("org", "Veilid", "Veilid") {
            default_config_path = PathBuf::from(my_proj_dirs.data_local_dir());
        } else {
            default_config_path = PathBuf::from("./");
        }
        default_config_path.push("protected_store");

        default_config_path
    }

    pub fn get_core_config_callback(&self) -> veilid_core::ConfigCallback {
        let inner = self.inner.clone();

        Arc::new(move |key: String| {
            let inner = inner.read();
            let out: Result<Box<dyn core::any::Any + Send>, String> = match key.as_str() {
                "program_name" => Ok(Box::new("veilid-server".to_owned())),
                "namespace" => Ok(Box::new(if inner.testing.subnode_index == 0 {
                    "".to_owned()
                } else {
                    format!("subnode{}", inner.testing.subnode_index)
                })),
                "api_log_level" => Ok(Box::new(veilid_core::VeilidConfigLogLevel::Off)),
                "capabilities.protocol_udp" => Ok(Box::new(true)),
                "capabilities.protocol_connect_tcp" => Ok(Box::new(true)),
                "capabilities.protocol_accept_tcp" => Ok(Box::new(true)),
                "capabilities.protocol_connect_ws" => Ok(Box::new(true)),
                "capabilities.protocol_accept_ws" => Ok(Box::new(true)),
                "capabilities.protocol_connect_wss" => Ok(Box::new(true)),
                "capabilities.protocol_accept_wss" => Ok(Box::new(true)),
                "protected_store.allow_insecure_fallback" => {
                    Ok(Box::new(inner.core.protected_store.allow_insecure_fallback))
                }
                "protected_store.always_use_insecure_storage" => Ok(Box::new(
                    inner.core.protected_store.always_use_insecure_storage,
                )),
                "protected_store.insecure_fallback_directory" => Ok(Box::new(
                    inner
                        .core
                        .protected_store
                        .insecure_fallback_directory
                        .to_string_lossy()
                        .to_string(),
                )),
                "protected_store.delete" => Ok(Box::new(inner.core.protected_store.delete)),

                "table_store.directory" => Ok(Box::new(
                    inner
                        .core
                        .table_store
                        .directory
                        .to_string_lossy()
                        .to_string(),
                )),
                "table_store.delete" => Ok(Box::new(inner.core.table_store.delete)),

                "block_store.directory" => Ok(Box::new(
                    inner
                        .core
                        .block_store
                        .directory
                        .to_string_lossy()
                        .to_string(),
                )),
                "block_store.delete" => Ok(Box::new(inner.core.block_store.delete)),

                "network.max_connections" => Ok(Box::new(inner.core.network.max_connections)),
                "network.connection_initial_timeout" => {
                    Ok(Box::new(inner.core.network.connection_initial_timeout))
                }
                "network.node_id" => Ok(Box::new(inner.core.network.node_id)),
                "network.node_id_secret" => Ok(Box::new(inner.core.network.node_id_secret)),
                "network.bootstrap" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .bootstrap
                        .clone()
                        .into_iter()
                        .map(|e| e.node_dial_info_string)
                        .collect::<Vec<String>>(),
                )),
                "network.rpc.concurrency" => Ok(Box::new(inner.core.network.rpc.concurrency)),
                "network.rpc.queue_size" => Ok(Box::new(inner.core.network.rpc.queue_size)),
                "network.rpc.max_timestamp_behind" => {
                    Ok(Box::new(inner.core.network.rpc.max_timestamp_behind))
                }
                "network.rpc.max_timestamp_ahead" => {
                    Ok(Box::new(inner.core.network.rpc.max_timestamp_ahead))
                }
                "network.rpc.timeout" => Ok(Box::new(inner.core.network.rpc.timeout)),
                "network.rpc.max_route_hop_count" => {
                    Ok(Box::new(inner.core.network.rpc.max_route_hop_count))
                }
                "network.dht.resolve_node_timeout" => {
                    Ok(Box::new(inner.core.network.dht.resolve_node_timeout))
                }
                "network.dht.resolve_node_count" => {
                    Ok(Box::new(inner.core.network.dht.resolve_node_count))
                }
                "network.dht.resolve_node_fanout" => {
                    Ok(Box::new(inner.core.network.dht.resolve_node_fanout))
                }
                "network.dht.max_find_node_count" => {
                    Ok(Box::new(inner.core.network.dht.max_find_node_count))
                }
                "network.dht.get_value_timeout" => {
                    Ok(Box::new(inner.core.network.dht.get_value_timeout))
                }
                "network.dht.get_value_count" => {
                    Ok(Box::new(inner.core.network.dht.get_value_count))
                }
                "network.dht.get_value_fanout" => {
                    Ok(Box::new(inner.core.network.dht.get_value_fanout))
                }
                "network.dht.set_value_timeout" => {
                    Ok(Box::new(inner.core.network.dht.set_value_timeout))
                }
                "network.dht.set_value_count" => {
                    Ok(Box::new(inner.core.network.dht.set_value_count))
                }
                "network.dht.set_value_fanout" => {
                    Ok(Box::new(inner.core.network.dht.set_value_fanout))
                }
                "network.dht.min_peer_count" => Ok(Box::new(inner.core.network.dht.min_peer_count)),
                "network.dht.min_peer_refresh_time" => {
                    Ok(Box::new(inner.core.network.dht.min_peer_refresh_time))
                }
                "network.dht.validate_dial_info_receipt_time" => Ok(Box::new(
                    inner.core.network.dht.validate_dial_info_receipt_time,
                )),
                "network.upnp" => Ok(Box::new(inner.core.network.upnp)),
                "network.natpmp" => Ok(Box::new(inner.core.network.natpmp)),
                "network.enable_local_peer_scope" => {
                    Ok(Box::new(inner.core.network.enable_local_peer_scope))
                }
                "network.restricted_nat_retries" => {
                    Ok(Box::new(inner.core.network.restricted_nat_retries))
                }
                "network.tls.certificate_path" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .tls
                        .certificate_path
                        .to_string_lossy()
                        .to_string(),
                )),
                "network.tls.private_key_path" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .tls
                        .private_key_path
                        .to_string_lossy()
                        .to_string(),
                )),
                "network.tls.connection_initial_timeout" => {
                    Ok(Box::new(inner.core.network.tls.connection_initial_timeout))
                }
                "network.application.https.enabled" => {
                    Ok(Box::new(inner.core.network.application.https.enabled))
                }
                "network.application.https.listen_address" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .application
                        .https
                        .listen_address
                        .name
                        .clone(),
                )),
                "network.application.https.path" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .application
                        .https
                        .path
                        .to_string_lossy()
                        .to_string(),
                )),
                "network.application.https.url" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .application
                        .https
                        .url
                        .as_ref()
                        .map(|a| a.urlstring.clone()),
                )),
                "network.application.http.enabled" => {
                    Ok(Box::new(inner.core.network.application.http.enabled))
                }
                "network.application.http.listen_address" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .application
                        .http
                        .listen_address
                        .name
                        .clone(),
                )),
                "network.application.http.path" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .application
                        .http
                        .path
                        .to_string_lossy()
                        .to_string(),
                )),
                "network.application.http.url" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .application
                        .http
                        .url
                        .as_ref()
                        .map(|a| a.urlstring.clone()),
                )),
                "network.protocol.udp.enabled" => {
                    Ok(Box::new(inner.core.network.protocol.udp.enabled))
                }
                "network.protocol.udp.socket_pool_size" => {
                    Ok(Box::new(inner.core.network.protocol.udp.socket_pool_size))
                }
                "network.protocol.udp.listen_address" => Ok(Box::new(
                    inner.core.network.protocol.udp.listen_address.name.clone(),
                )),
                "network.protocol.udp.public_address" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .protocol
                        .udp
                        .public_address
                        .as_ref()
                        .map(|a| a.name.clone()),
                )),
                "network.protocol.tcp.connect" => {
                    Ok(Box::new(inner.core.network.protocol.tcp.connect))
                }
                "network.protocol.tcp.listen" => {
                    Ok(Box::new(inner.core.network.protocol.tcp.listen))
                }
                "network.protocol.tcp.max_connections" => {
                    Ok(Box::new(inner.core.network.protocol.tcp.max_connections))
                }
                "network.protocol.tcp.listen_address" => Ok(Box::new(
                    inner.core.network.protocol.tcp.listen_address.name.clone(),
                )),
                "network.protocol.tcp.public_address" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .protocol
                        .tcp
                        .public_address
                        .as_ref()
                        .map(|a| a.name.clone()),
                )),
                "network.protocol.ws.connect" => {
                    Ok(Box::new(inner.core.network.protocol.ws.connect))
                }
                "network.protocol.ws.listen" => Ok(Box::new(inner.core.network.protocol.ws.listen)),
                "network.protocol.ws.max_connections" => {
                    Ok(Box::new(inner.core.network.protocol.ws.max_connections))
                }
                "network.protocol.ws.listen_address" => Ok(Box::new(
                    inner.core.network.protocol.ws.listen_address.name.clone(),
                )),
                "network.protocol.ws.path" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .protocol
                        .ws
                        .path
                        .to_string_lossy()
                        .to_string(),
                )),
                "network.protocol.ws.url" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .protocol
                        .ws
                        .url
                        .as_ref()
                        .map(|a| a.urlstring.clone()),
                )),
                "network.protocol.wss.connect" => {
                    Ok(Box::new(inner.core.network.protocol.wss.connect))
                }
                "network.protocol.wss.listen" => {
                    Ok(Box::new(inner.core.network.protocol.wss.listen))
                }
                "network.protocol.wss.max_connections" => {
                    Ok(Box::new(inner.core.network.protocol.wss.max_connections))
                }
                "network.protocol.wss.listen_address" => Ok(Box::new(
                    inner.core.network.protocol.wss.listen_address.name.clone(),
                )),
                "network.protocol.wss.path" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .protocol
                        .wss
                        .path
                        .to_string_lossy()
                        .to_string(),
                )),
                "network.protocol.wss.url" => Ok(Box::new(
                    inner
                        .core
                        .network
                        .protocol
                        .wss
                        .url
                        .as_ref()
                        .map(|a| a.urlstring.clone()),
                )),
                "network.leases.max_server_signal_leases" => {
                    Ok(Box::new(inner.core.network.leases.max_server_signal_leases))
                }
                "network.leases.max_server_relay_leases" => {
                    Ok(Box::new(inner.core.network.leases.max_server_relay_leases))
                }
                "network.leases.max_client_signal_leases" => {
                    Ok(Box::new(inner.core.network.leases.max_client_signal_leases))
                }
                "network.leases.max_client_relay_leases" => {
                    Ok(Box::new(inner.core.network.leases.max_client_relay_leases))
                }
                _ => Err(format!("config key '{}' doesn't exist", key)),
            };
            out
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_default_config() {
        let mut cfg = config::Config::default();
        load_default_config(&mut cfg).unwrap();
        let inner = cfg.try_into::<SettingsInner>().unwrap();
        println!("default settings: {:?}", inner);
    }

    #[test]
    #[serial]
    fn test_default_config_settings() {
        let settings = Settings::new(true, OsStr::new("!!!")).unwrap();

        let s = settings.read();
        assert_eq!(s.daemon, false);
        assert_eq!(s.client_api.enabled, true);
        assert_eq!(s.client_api.listen_address.name, "localhost:5959");
        assert_eq!(
            s.client_api.listen_address.addrs,
            "localhost:5959"
                .to_socket_addrs()
                .unwrap()
                .collect::<Vec<SocketAddr>>()
        );
        assert_eq!(s.auto_attach, true);
        assert_eq!(s.logging.terminal.enabled, true);
        assert_eq!(s.logging.terminal.level, LogLevel::Info);
        assert_eq!(s.logging.file.enabled, false);
        assert_eq!(s.logging.file.path, "");
        assert_eq!(s.logging.file.append, true);
        assert_eq!(s.logging.file.level, LogLevel::Info);
        assert_eq!(s.logging.client.enabled, true);
        assert_eq!(s.logging.client.level, LogLevel::Info);
        assert_eq!(s.testing.subnode_index, 0);

        assert_eq!(
            s.core.table_store.directory,
            Settings::get_default_table_store_path()
        );
        assert_eq!(s.core.table_store.delete, false);

        assert_eq!(
            s.core.block_store.directory,
            Settings::get_default_block_store_path()
        );
        assert_eq!(s.core.block_store.delete, false);

        assert_eq!(s.core.protected_store.allow_insecure_fallback, true);
        assert_eq!(s.core.protected_store.always_use_insecure_storage, false);
        assert_eq!(
            s.core.protected_store.insecure_fallback_directory,
            Settings::get_default_protected_store_insecure_fallback_directory()
        );
        assert_eq!(s.core.protected_store.delete, false);

        assert_eq!(s.core.network.max_connections, 16);
        assert_eq!(s.core.network.connection_initial_timeout, 2_000_000u64);
        assert_eq!(s.core.network.node_id, veilid_core::DHTKey::default());
        assert_eq!(
            s.core.network.node_id_secret,
            veilid_core::DHTKeySecret::default()
        );
        //
        assert!(s.core.network.bootstrap.is_empty());
        //
        assert_eq!(s.core.network.rpc.concurrency, 0);
        assert_eq!(s.core.network.rpc.queue_size, 1024);
        assert_eq!(s.core.network.rpc.max_timestamp_behind, Some(10_000_000u64));
        assert_eq!(s.core.network.rpc.max_timestamp_ahead, Some(10_000_000u64));
        assert_eq!(s.core.network.rpc.timeout, 10000000);
        assert_eq!(s.core.network.rpc.max_route_hop_count, 7);
        //
        assert_eq!(s.core.network.dht.resolve_node_timeout, None);
        assert_eq!(s.core.network.dht.resolve_node_count, 20u32);
        assert_eq!(s.core.network.dht.resolve_node_fanout, 3u32);
        assert_eq!(s.core.network.dht.max_find_node_count, 20u32);
        assert_eq!(s.core.network.dht.get_value_timeout, None);
        assert_eq!(s.core.network.dht.get_value_count, 20u32);
        assert_eq!(s.core.network.dht.get_value_fanout, 3u32);
        assert_eq!(s.core.network.dht.set_value_timeout, None);
        assert_eq!(s.core.network.dht.set_value_count, 20u32);
        assert_eq!(s.core.network.dht.set_value_fanout, 5u32);
        assert_eq!(s.core.network.dht.min_peer_count, 20u32);
        assert_eq!(s.core.network.dht.min_peer_refresh_time, 2000000u64);
        assert_eq!(
            s.core.network.dht.validate_dial_info_receipt_time,
            5000000u64
        );
        //
        assert_eq!(s.core.network.upnp, false);
        assert_eq!(s.core.network.natpmp, false);
        assert_eq!(s.core.network.enable_local_peer_scope, false);
        assert_eq!(s.core.network.restricted_nat_retries, 3u32);
        //
        assert_eq!(
            s.core.network.tls.certificate_path,
            std::path::PathBuf::from("/etc/veilid/server.crt")
        );
        assert_eq!(
            s.core.network.tls.private_key_path,
            std::path::PathBuf::from("/etc/veilid/private/server.key")
        );
        assert_eq!(s.core.network.tls.connection_initial_timeout, 2_000_000u64);
        //
        assert_eq!(s.core.network.application.https.enabled, false);
        assert_eq!(
            s.core.network.application.https.listen_address.name,
            "[::]:5150"
        );
        assert_eq!(
            s.core.network.application.https.listen_address.addrs,
            "[::]:5150"
                .to_socket_addrs()
                .unwrap()
                .collect::<Vec<SocketAddr>>()
        );
        assert_eq!(
            s.core.network.application.https.path,
            std::path::PathBuf::from("app")
        );
        assert_eq!(s.core.network.application.https.url, None);
        assert_eq!(s.core.network.application.http.enabled, false);
        assert_eq!(
            s.core.network.application.http.listen_address.name,
            "[::]:5150"
        );
        assert_eq!(
            s.core.network.application.http.listen_address.addrs,
            "[::]:5150"
                .to_socket_addrs()
                .unwrap()
                .collect::<Vec<SocketAddr>>()
        );
        assert_eq!(
            s.core.network.application.http.path,
            std::path::PathBuf::from("app")
        );
        assert_eq!(s.core.network.application.http.url, None);
        //
        assert_eq!(s.core.network.protocol.udp.enabled, true);
        assert_eq!(s.core.network.protocol.udp.socket_pool_size, 0);
        assert_eq!(s.core.network.protocol.udp.listen_address.name, "[::]:5150");
        assert_eq!(
            s.core.network.protocol.udp.listen_address.addrs,
            "[::]:5150"
                .to_socket_addrs()
                .unwrap()
                .collect::<Vec<SocketAddr>>()
        );
        assert_eq!(s.core.network.protocol.udp.public_address, None);

        //
        assert_eq!(s.core.network.protocol.tcp.connect, true);
        assert_eq!(s.core.network.protocol.tcp.listen, true);
        assert_eq!(s.core.network.protocol.tcp.max_connections, 32);
        assert_eq!(s.core.network.protocol.tcp.listen_address.name, "[::]:5150");
        assert_eq!(
            s.core.network.protocol.tcp.listen_address.addrs,
            "[::]:5150"
                .to_socket_addrs()
                .unwrap()
                .collect::<Vec<SocketAddr>>()
        );
        assert_eq!(s.core.network.protocol.tcp.public_address, None);

        //
        assert_eq!(s.core.network.protocol.ws.connect, true);
        assert_eq!(s.core.network.protocol.ws.listen, true);
        assert_eq!(s.core.network.protocol.ws.max_connections, 16);
        assert_eq!(s.core.network.protocol.ws.listen_address.name, "[::]:5150");
        assert_eq!(
            s.core.network.protocol.ws.listen_address.addrs,
            "[::]:5150"
                .to_socket_addrs()
                .unwrap()
                .collect::<Vec<SocketAddr>>()
        );
        assert_eq!(
            s.core.network.protocol.ws.path,
            std::path::PathBuf::from("ws")
        );
        assert_eq!(s.core.network.protocol.ws.url, None);
        //
        assert_eq!(s.core.network.protocol.wss.connect, true);
        assert_eq!(s.core.network.protocol.wss.listen, false);
        assert_eq!(s.core.network.protocol.wss.max_connections, 16);
        assert_eq!(s.core.network.protocol.wss.listen_address.name, "[::]:5150");
        assert_eq!(
            s.core.network.protocol.wss.listen_address.addrs,
            "[::]:5150"
                .to_socket_addrs()
                .unwrap()
                .collect::<Vec<SocketAddr>>()
        );
        assert_eq!(
            s.core.network.protocol.wss.path,
            std::path::PathBuf::from("ws")
        );
        assert_eq!(s.core.network.protocol.wss.url, None);
        //
        assert_eq!(s.core.network.leases.max_server_signal_leases, 256);
        assert_eq!(s.core.network.leases.max_server_relay_leases, 8);
        assert_eq!(s.core.network.leases.max_client_signal_leases, 2);
        assert_eq!(s.core.network.leases.max_client_relay_leases, 2);
    }
}

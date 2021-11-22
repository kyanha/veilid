use config;
use directories::*;
use log;
use serde;
use serde_derive::*;
use std::ffi::OsStr;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

pub fn load_default_config(cfg: &mut config::Config) -> Result<(), config::ConfigError> {
    let default_config = r###"---
address: "localhost:5959"
autoconnect: true
autoreconnect: true
logging: 
    level: "info"
    terminal: 
        enabled: false
    file:
        enabled: true
        directory: ""
        append: true
interface:
    node_log:
        scrollback: 2048
    command_line:
        history_size: 2048
    theme:
        shadow: false
        borders: "simple"
        colors:
            background         : "#333D3D"
            shadow             : "#000000"
            view               : "#1c2323"
            primary            : "#a6d8d3"
            secondary          : "#8cb4b7"
            tertiary           : "#eeeeee"
            title_primary      : "#f93fbd"
            title_secondary    : "#ff0000"
            highlight          : "#f93fbd"
            highlight_inactive : "#a6d8d3"
            highlight_text     : "#333333"
        log_colors:
            trace              : "#707070"
            debug              : "#a0a0a0"
            info               : "#5cd3c6"
            warn               : "#fedc50"
            error              : "#ff4a15"
    "###;
    cfg.merge(config::File::from_str(
        default_config,
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

#[derive(Copy, Clone, Debug)]
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
pub fn convert_loglevel(log_level: LogLevel) -> log::LevelFilter {
    match log_level {
        LogLevel::Error => log::LevelFilter::Error,
        LogLevel::Warn => log::LevelFilter::Warn,
        LogLevel::Info => log::LevelFilter::Info,
        LogLevel::Debug => log::LevelFilter::Debug,
        LogLevel::Trace => log::LevelFilter::Trace,
    }
}

#[derive(Debug)]
pub struct NamedSocketAddrs {
    pub name: String,
    pub addrs: Vec<SocketAddr>,
}
impl<'de> serde::Deserialize<'de> for NamedSocketAddrs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let addr_iter = s
            .to_socket_addrs()
            .map_err(|x| serde::de::Error::custom(x))?;
        Ok(NamedSocketAddrs {
            name: s,
            addrs: addr_iter.collect(),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct Terminal {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct File {
    pub enabled: bool,
    pub directory: String,
    pub append: bool,
}

#[derive(Debug, Deserialize)]
pub struct Logging {
    pub terminal: Terminal,
    pub file: File,
    pub level: LogLevel,
}

#[derive(Debug, Deserialize)]
pub struct Colors {
    pub background: String,
    pub shadow: String,
    pub view: String,
    pub primary: String,
    pub secondary: String,
    pub tertiary: String,
    pub title_primary: String,
    pub title_secondary: String,
    pub highlight: String,
    pub highlight_inactive: String,
    pub highlight_text: String,
}

#[derive(Debug, Deserialize)]
pub struct LogColors {
    pub trace: String,
    pub debug: String,
    pub info: String,
    pub warn: String,
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct Theme {
    pub shadow: bool,
    pub borders: String,
    pub colors: Colors,
    pub log_colors: LogColors,
}

#[derive(Debug, Deserialize)]
pub struct NodeLog {
    pub scrollback: usize,
}

#[derive(Debug, Deserialize)]
pub struct CommandLine {
    pub history_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct Interface {
    pub theme: Theme,
    pub node_log: NodeLog,
    pub command_line: CommandLine,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub address: NamedSocketAddrs,
    pub autoconnect: bool,
    pub autoreconnect: bool,
    pub logging: Logging,
    pub interface: Interface,
}

impl Settings {
    pub fn get_default_config_path() -> PathBuf {
        // Get default configuration file location
        let mut default_config_path;

        if let Some(my_proj_dirs) = ProjectDirs::from("org", "Veilid", "Veilid") {
            default_config_path = PathBuf::from(my_proj_dirs.config_dir());
        } else {
            default_config_path = PathBuf::from("./");
        }
        default_config_path.push("veilid-client.conf");

        default_config_path
    }

    pub fn get_default_log_directory() -> PathBuf {
        // Get default configuration file location
        let mut default_log_directory;

        if let Some(my_proj_dirs) = ProjectDirs::from("org", "Veilid", "Veilid") {
            default_log_directory = PathBuf::from(my_proj_dirs.config_dir());
        } else {
            default_log_directory = PathBuf::from("./");
        }
        default_log_directory.push("logs/");

        default_log_directory
    }

    pub fn new(
        config_file_is_default: bool,
        config_file: &OsStr,
    ) -> Result<Self, config::ConfigError> {
        // Create a config
        let mut cfg = config::Config::default();

        // Load the default config
        load_default_config(&mut cfg)?;

        // Use default log directory for logs
        cfg.set(
            "logging.file.directory",
            Settings::get_default_log_directory().to_str(),
        )?;

        // Merge in the config file if we have one
        let config_file_path = Path::new(config_file);
        if !config_file_is_default || config_file_path.exists() {
            // If the user specifies a config file on the command line then it must exist
            load_config(&mut cfg, config_file_path)?;
        }
        cfg.try_into()
    }
}

#[test]
fn test_default_config() {
    let mut cfg = config::Config::default();

    load_default_config(&mut cfg).unwrap();
    let settings = cfg.try_into::<Settings>().unwrap();

    println!("default settings: {:?}", settings);
}

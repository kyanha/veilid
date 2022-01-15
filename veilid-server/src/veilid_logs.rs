use crate::client_log_channel::*;
use crate::settings::*;
use simplelog::*;
use std::fs::OpenOptions;
use std::path::Path;

pub struct VeilidLogs {
    pub client_log_channel: Option<ClientLogChannel>,
    pub client_log_channel_closer: Option<ClientLogChannelCloser>,
}

impl VeilidLogs {
    pub fn setup_normal_logs(settings: Settings) -> Result<VeilidLogs, String> {
        let settingsr = settings.read();

        // Set up loggers
        let mut logs: Vec<Box<dyn SharedLogger>> = Vec::new();
        let mut client_log_channel: Option<ClientLogChannel> = None;
        let mut client_log_channel_closer: Option<ClientLogChannelCloser> = None;
        let mut cb = ConfigBuilder::new();
        cb.add_filter_ignore_str("async_std");
        cb.add_filter_ignore_str("async_io");
        cb.add_filter_ignore_str("polling");
        cb.add_filter_ignore_str("rustls");
        cb.add_filter_ignore_str("async_tungstenite");
        cb.add_filter_ignore_str("tungstenite");
        cb.add_filter_ignore_str("netlink_proto");
        cb.add_filter_ignore_str("netlink_sys");

        if settingsr.logging.terminal.enabled {
            logs.push(TermLogger::new(
                convert_loglevel(settingsr.logging.terminal.level),
                cb.build(),
                TerminalMode::Mixed,
                ColorChoice::Auto,
            ))
        }
        if settingsr.logging.file.enabled {
            let log_path = Path::new(&settingsr.logging.file.path);

            let logfile;
            if settingsr.logging.file.append {
                logfile = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                    .map_err(|e| format!("failed to open log file: {}", e))?
            } else {
                logfile = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(log_path)
                    .map_err(|e| format!("failed to open log file: {}", e))?
            }
            logs.push(WriteLogger::new(
                convert_loglevel(settingsr.logging.file.level),
                cb.build(),
                logfile,
            ))
        }
        if settingsr.logging.client.enabled {
            let (clog, clogwriter, clogcloser) = ClientLogChannel::new();
            client_log_channel = Some(clog);
            client_log_channel_closer = Some(clogcloser);
            logs.push(WriteLogger::new(
                convert_loglevel(settingsr.logging.client.level),
                cb.build(),
                clogwriter,
            ))
        }

        CombinedLogger::init(logs).map_err(|e| format!("failed to init logs: {}", e))?;

        Ok(VeilidLogs {
            client_log_channel,
            client_log_channel_closer,
        })
    }
}

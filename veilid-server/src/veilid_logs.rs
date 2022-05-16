use crate::log_safe_channel::*;
use crate::settings::*;
use cfg_if::*;
use log::*;
use simplelog::*;
use std::fs::OpenOptions;
use std::path::Path;

pub struct VeilidLogs {
    pub client_log_channel: Option<LogSafeChannel>,
    pub client_log_channel_closer: Option<LogSafeChannelCloser>,
}

cfg_if! {
    if #[cfg(target_os = "linux")] {
        use systemd_journal_logger::JournalLog;
        pub struct SystemLogger {
            level_filter: LevelFilter,
            config: Config,
            journal_log: JournalLog<String,String>,
        }

        impl SystemLogger {
            pub fn new(level_filter: LevelFilter, config: Config) -> Box<Self> {
                Box::new(Self {
                    level_filter,
                    config,
                    journal_log: JournalLog::with_extra_fields(Vec::new())
                })
            }

            pub fn should_skip(record: &Record<'_>) -> bool {
                // // If a module path and allowed list are available
                // match (record.target(), &*config.filter_allow) {
                //     (path, allowed) if !allowed.is_empty() => {
                //         // Check that the module path matches at least one allow filter
                //         if !allowed.iter().any(|v| path.starts_with(&**v)) {
                //             // If not, skip any further writing
                //             return true;
                //         }
                //     }
                //     _ => {}
                // }

                // If a module path and ignore list are available
                match (record.target(), &veilid_core::DEFAULT_LOG_IGNORE_LIST) {
                    (path, ignore) if !ignore.is_empty() => {
                        // Check that the module path does not match any ignore filters
                        if ignore.iter().any(|v| path.starts_with(&**v)) {
                            // If not, skip any further writing
                            return true;
                        }
                    }
                    _ => {}
                }

                false
            }

        }

        impl Log for SystemLogger {
            fn enabled(&self, metadata: &Metadata<'_>) -> bool {
                metadata.level() <= self.level_filter
            }

            fn log(&self, record: &Record<'_>) {
                if self.enabled(record.metadata()) && ! Self::should_skip(record) {
                    self.journal_log.log(record);
                }
            }

            fn flush(&self) {
                self.journal_log.flush();
            }
        }

        impl SharedLogger for SystemLogger {
            fn level(&self) -> LevelFilter {
                self.level_filter
            }
            fn config(&self) -> Option<&Config> {
                Some(&self.config)
            }
            fn as_log(self: Box<Self>) -> Box<dyn Log> {
                Box::new(*self)
            }
        }
    }
}

impl VeilidLogs {
    pub fn setup_normal_logs(settings: Settings) -> Result<VeilidLogs, String> {
        let settingsr = settings.read();

        // Set up loggers
        let mut logs: Vec<Box<dyn SharedLogger>> = Vec::new();
        let mut client_log_channel: Option<LogSafeChannel> = None;
        let mut client_log_channel_closer: Option<LogSafeChannelCloser> = None;
        let mut cb = ConfigBuilder::new();
        for ig in veilid_core::DEFAULT_LOG_IGNORE_LIST {
            cb.add_filter_ignore_str(ig);
        }

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

            let logfile = if settingsr.logging.file.append {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                    .map_err(|e| format!("failed to open log file: {}", e))?
            } else {
                OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(log_path)
                    .map_err(|e| format!("failed to open log file: {}", e))?
            };
            logs.push(WriteLogger::new(
                convert_loglevel(settingsr.logging.file.level),
                cb.build(),
                logfile,
            ))
        }
        if settingsr.logging.client.enabled {
            let (clog, clogwriter, clogcloser) = LogSafeChannel::new();
            client_log_channel = Some(clog);
            client_log_channel_closer = Some(clogcloser);
            logs.push(WriteLogger::new(
                convert_loglevel(settingsr.logging.client.level),
                cb.build(),
                clogwriter,
            ))
        }

        cfg_if! {
            if #[cfg(target_os = "linux")] {
                if settingsr.logging.system.enabled {
                    logs.push(SystemLogger::new(convert_loglevel(settingsr.logging.system.level), cb.build()))
                }
            }
        }

        CombinedLogger::init(logs).map_err(|e| format!("failed to init logs: {}", e))?;

        Ok(VeilidLogs {
            client_log_channel,
            client_log_channel_closer,
        })
    }
}

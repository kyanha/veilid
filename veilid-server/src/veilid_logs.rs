use crate::settings::*;
use cfg_if::*;
use std::path::*;
use tracing::*;
use tracing_appender::*;
use tracing_subscriber::prelude::*;
use tracing_subscriber::*;

pub struct VeilidLogs {
    pub guard: Option<non_blocking::WorkerGuard>,
}

fn logfilter<T: AsRef<str>, V: AsRef<[T]>>(
    metadata: &Metadata,
    max_level: veilid_core::VeilidLogLevel,
    ignore_list: V,
) -> bool {
    // Skip things out of level
    let log_level = veilid_core::VeilidLogLevel::from_tracing_level(*metadata.level());
    if log_level <= max_level {
        return true;
    }

    // Skip filtered targets
    !match (metadata.target(), ignore_list.as_ref()) {
        (path, ignore) if !ignore.is_empty() => {
            // Check that the module path does not match any ignore filters
            ignore.iter().any(|v| path.starts_with(v.as_ref()))
        }
        _ => false,
    }
}

impl VeilidLogs {
    pub fn setup(settings: Settings) -> Result<VeilidLogs, String> {
        let settingsr = settings.read();

        // Set up subscriber and layers

        let mut ignore_list = Vec::<String>::new();
        for ig in veilid_core::DEFAULT_LOG_IGNORE_LIST {
            ignore_list.push(ig.to_owned());
        }

        let subscriber = Registry::default();

        let subscriber = subscriber.with(
            EnvFilter::builder()
                .with_default_directive(level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        );

        let subscriber = subscriber.with(if settingsr.logging.terminal.enabled {
            let terminal_max_log_level = convert_loglevel(settingsr.logging.terminal.level);
            let ignore_list = ignore_list.clone();
            Some(
                fmt::Layer::new()
                    .compact()
                    .with_writer(std::io::stdout)
                    .with_filter(filter::FilterFn::new(move |metadata| {
                        logfilter(metadata, terminal_max_log_level, &ignore_list)
                    })),
            )
        } else {
            None
        });

        let mut guard = None;
        let subscriber = subscriber.with(if settingsr.logging.file.enabled {
            let file_max_log_level = convert_loglevel(settingsr.logging.file.level);

            let log_path = Path::new(&settingsr.logging.file.path);
            let full_path = std::env::current_dir()
                .unwrap_or(PathBuf::from(MAIN_SEPARATOR.to_string()))
                .join(log_path);
            let log_parent = full_path
                .parent()
                .unwrap_or(Path::new(&MAIN_SEPARATOR.to_string()))
                .canonicalize()
                .map_err(|e| {
                    format!(
                        "File log path parent does not exist: {} ({})",
                        settingsr.logging.file.path, e
                    )
                })?;
            let log_filename = full_path.file_name().ok_or(format!(
                "File log filename not specified in path: {}",
                settingsr.logging.file.path
            ))?;

            let appender = tracing_appender::rolling::never(log_parent, Path::new(log_filename));
            let (non_blocking_appender, non_blocking_guard) =
                tracing_appender::non_blocking(appender);
            guard = Some(non_blocking_guard);

            let ignore_list = ignore_list.clone();
            Some(
                fmt::Layer::new()
                    .compact()
                    .with_writer(non_blocking_appender)
                    .with_filter(filter::FilterFn::new(move |metadata| {
                        logfilter(metadata, file_max_log_level, &ignore_list)
                    })),
            )
        } else {
            None
        });

        let subscriber = subscriber.with(if settingsr.logging.api.enabled {
            // Get layer from veilid core, filtering is done by ApiTracingLayer automatically
            Some(veilid_core::ApiTracingLayer::get())
        } else {
            None
        });

        cfg_if! {
            if #[cfg(target_os = "linux")] {
                let subscriber = subscriber.with(if settingsr.logging.system.enabled {
                    let ignore_list = ignore_list.clone();
                    let system_max_log_level = convert_loglevel(settingsr.logging.system.level);
                    Some(tracing_journald::layer().map_err(|e| format!("failed to set up journald logging: {}", e))?.with_filter(filter::FilterFn::new(move |metadata| {
                        logfilter(metadata, system_max_log_level, &ignore_list)
                    })))
                } else {
                    None
                });
            }
        }

        subscriber
            .try_init()
            .map_err(|e| format!("failed to initialize logging: {}", e))?;

        Ok(VeilidLogs { guard })
    }
}

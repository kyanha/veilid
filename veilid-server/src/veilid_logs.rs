use crate::settings::*;
use cfg_if::*;
use opentelemetry_otlp::WithExportConfig;
use std::path::*;
use tracing::*;
use tracing_appender::*;
use tracing_subscriber::prelude::*;
use tracing_subscriber::*;

pub struct VeilidLogs {
    pub guard: Option<non_blocking::WorkerGuard>,
}

fn logfilter<T: AsRef<str>, V: AsRef<[T]>>(metadata: &Metadata, ignore_list: V) -> bool {
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

        // Terminal logger
        let subscriber = subscriber.with(if settingsr.logging.terminal.enabled {
            let terminal_max_log_level: level_filters::LevelFilter =
                convert_loglevel(settingsr.logging.terminal.level)
                    .to_tracing_level()
                    .into();
            let ignore_list = ignore_list.clone();
            Some(
                fmt::Layer::new()
                    .compact()
                    .with_writer(std::io::stdout)
                    .with_filter(terminal_max_log_level)
                    .with_filter(filter::FilterFn::new(move |metadata| {
                        logfilter(metadata, &ignore_list)
                    })),
            )
        } else {
            None
        });

        // OpenTelemetry logger
        let subscriber = subscriber.with(if settingsr.logging.otlp.enabled {
            let otlp_max_log_level: level_filters::LevelFilter =
                convert_loglevel(settingsr.logging.otlp.level)
                    .to_tracing_level()
                    .into();
            let grpc_endpoint = match &settingsr.logging.otlp.grpc_endpoint {
                Some(v) => &v.urlstring,
                None => {
                    return Err("missing OTLP GRPC endpoint url".to_owned());
                }
            };

            // Required for GRPC dns resolution to work
            std::env::set_var("GRPC_DNS_RESOLVER", "native");

            let tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .grpcio()
                        .with_endpoint(grpc_endpoint),
                )
                .install_batch(opentelemetry::runtime::AsyncStd)
                .map_err(|e| format!("failed to install OpenTelemetry tracer: {}", e))?;

            let ignore_list = ignore_list.clone();
            Some(
                tracing_opentelemetry::layer()
                    .with_tracer(tracer)
                    .with_filter(otlp_max_log_level)
                    .with_filter(filter::FilterFn::new(move |metadata| {
                        logfilter(metadata, &ignore_list)
                    })),
            )
        } else {
            None
        });

        // File logger
        let mut guard = None;
        let subscriber = subscriber.with(if settingsr.logging.file.enabled {
            let file_max_log_level: level_filters::LevelFilter =
                convert_loglevel(settingsr.logging.file.level)
                    .to_tracing_level()
                    .into();

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
                    .with_filter(file_max_log_level)
                    .with_filter(filter::FilterFn::new(move |metadata| {
                        logfilter(metadata, &ignore_list)
                    })),
            )
        } else {
            None
        });

        // API logger
        let subscriber = subscriber.with(if settingsr.logging.api.enabled {
            // Get layer from veilid core, filtering is done by ApiTracingLayer automatically
            Some(veilid_core::ApiTracingLayer::get())
        } else {
            None
        });

        // Systemd Journal logger
        cfg_if! {
            if #[cfg(target_os = "linux")] {
                let subscriber = subscriber.with(if settingsr.logging.system.enabled {
                    let ignore_list = ignore_list.clone();
                    let system_max_log_level: level_filters::LevelFilter = convert_loglevel(settingsr.logging.system.level).to_tracing_level().into();
                    Some(tracing_journald::layer().map_err(|e| format!("failed to set up journald logging: {}", e))?
                        .with_filter(system_max_log_level)
                        .with_filter(filter::FilterFn::new(move |metadata| {
                            logfilter(metadata, &ignore_list)
                        }))
                    )
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

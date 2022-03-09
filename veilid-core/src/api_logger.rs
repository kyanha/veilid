use crate::core_context::*;
use crate::intf::*;
use crate::veilid_api::*;
use crate::xx::*;
use log::{set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};
use once_cell::sync::OnceCell;

struct ApiLoggerInner {
    level: LevelFilter,
    filter_ignore: Cow<'static, [Cow<'static, str>]>,
    join_handle: Option<JoinHandle<()>>,
    tx: async_channel::Sender<(VeilidLogLevel, String)>,
}

#[derive(Clone)]
pub struct ApiLogger {
    inner: Arc<Mutex<Option<ApiLoggerInner>>>,
}

static API_LOGGER: OnceCell<ApiLogger> = OnceCell::new();

impl ApiLogger {
    fn new_inner(level: LevelFilter, update_callback: UpdateCallback) -> ApiLoggerInner {
        let (tx, rx) = async_channel::unbounded::<(VeilidLogLevel, String)>();
        let join_handle: Option<JoinHandle<()>> = Some(spawn(async move {
            while let Ok(v) = rx.recv().await {
                (update_callback)(VeilidUpdate::Log {
                    log_level: v.0,
                    message: v.1,
                })
                .await;
            }
        }));
        ApiLoggerInner {
            level,
            filter_ignore: Default::default(),
            join_handle,
            tx,
        }
    }

    pub async fn init(log_level: LevelFilter, update_callback: UpdateCallback) {
        set_max_level(log_level);
        let api_logger = API_LOGGER.get_or_init(|| {
            let api_logger = ApiLogger {
                inner: Arc::new(Mutex::new(None)),
            };
            set_boxed_logger(Box::new(api_logger.clone())).expect("failed to set api logger");
            api_logger
        });
        let apilogger_inner = Some(Self::new_inner(log_level, update_callback));
        *api_logger.inner.lock() = apilogger_inner;
    }

    pub async fn terminate() {
        if let Some(api_logger) = API_LOGGER.get() {
            let mut join_handle = None;
            {
                let mut inner = api_logger.inner.lock();

                // Terminate channel
                if let Some(inner) = (*inner).as_mut() {
                    inner.tx.close();
                    join_handle = inner.join_handle.take();
                }
                *inner = None;
            }
            if let Some(jh) = join_handle {
                jh.await;
            }

            // Clear everything and we're done
            set_max_level(LevelFilter::Off);
        }
    }

    pub fn change_log_level(log_level: LevelFilter) {
        if let Some(api_logger) = API_LOGGER.get() {
            if let Some(inner) = &mut *api_logger.inner.lock() {
                set_max_level(log_level);
                inner.level = log_level;
            }
        }
    }

    pub fn add_filter_ignore_str(filter_ignore: &'static str) {
        if let Some(api_logger) = API_LOGGER.get() {
            if let Some(inner) = &mut *api_logger.inner.lock() {
                let mut list = Vec::from(&*inner.filter_ignore);
                list.push(Cow::Borrowed(filter_ignore));
                inner.filter_ignore = Cow::Owned(list);
            }
        }
    }
}

impl Log for ApiLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        if let Some(inner) = &mut *self.inner.lock() {
            return metadata.level() <= inner.level;
        }
        false
    }

    fn log(&self, record: &Record<'_>) {
        if let Some(inner) = &mut *self.inner.lock() {
            // Skip filtered targets
            let skip = match (record.target(), &*inner.filter_ignore) {
                (path, ignore) if !ignore.is_empty() => {
                    // Check that the module path does not match any ignore filters
                    ignore.iter().any(|v| path.starts_with(&**v))
                }
                _ => false,
            };
            if skip {
                return;
            }

            let metadata = record.metadata();
            let level = metadata.level();
            if level <= inner.level {
                let ll = VeilidLogLevel::from_log_level(level);

                let file = record.file().unwrap_or("<unknown>");
                let loc = if level >= Level::Debug {
                    if let Some(line) = record.line() {
                        format!("[{}:{}] ", file, line)
                    } else {
                        format!("[{}:<unknown>] ", file)
                    }
                } else {
                    "".to_owned()
                };
                let tgt = if record.target().is_empty() {
                    "".to_owned()
                } else {
                    format!("{}: ", record.target())
                };

                let s = format!("{}{}{}", tgt, loc, record.args());

                let _ = inner.tx.try_send((ll, s));
            }
        }
    }

    fn flush(&self) {
        // always flushes
    }
}

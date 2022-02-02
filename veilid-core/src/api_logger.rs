use crate::intf::*;
use crate::veilid_api::*;
use crate::veilid_core::*;
use crate::xx::*;
use log::{set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record};
use once_cell::sync::OnceCell;

struct ApiLoggerInner {
    level: LevelFilter,
    filter_ignore: Cow<'static, [Cow<'static, str>]>,
    _join_handle: JoinHandle<()>,
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
        let _join_handle: JoinHandle<()> = spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(v) => {
                        (update_callback)(VeilidUpdate::Log {
                            log_level: v.0,
                            message: v.1,
                        })
                        .await;
                    }
                    Err(_) => {
                        // Nothing to be done here...
                        break;
                    }
                }
            }
        });
        ApiLoggerInner {
            level,
            filter_ignore: Default::default(),
            _join_handle,
            tx,
        }
    }

    pub fn init(log_level: LevelFilter, update_callback: UpdateCallback) {
        set_max_level(log_level);
        let api_logger = API_LOGGER.get_or_init(|| {
            let api_logger = ApiLogger {
                inner: Arc::new(Mutex::new(None)),
            };
            set_boxed_logger(Box::new(api_logger.clone())).expect("failed to set api logger");
            api_logger
        });

        let mut inner = api_logger.inner.lock();
        *inner = Some(Self::new_inner(log_level, update_callback));
    }

    pub fn terminate() {
        if let Some(api_logger) = API_LOGGER.get() {
            let mut inner = api_logger.inner.lock();
            *inner = None;
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

use crate::dart_isolate_wrapper::*;
use allo_isolate::*;
use async_std::sync::Mutex as AsyncMutex;
use ffi_support::*;
use lazy_static::*;
use std::os::raw::c_char;
use std::sync::Arc;
use tracing::*;

// Globals

lazy_static! {
    static ref VEILID_API: AsyncMutex<Option<veilid_core::VeilidAPI>> = AsyncMutex::new(None);
}

async fn get_veilid_api() -> Result<veilid_core::VeilidAPI, veilid_core::VeilidAPIError> {
    let api_lock = VEILID_API.lock().await;
    api_lock
        .as_ref()
        .cloned()
        .ok_or(veilid_core::VeilidAPIError::NotInitialized)
}

async fn take_veilid_api() -> Result<veilid_core::VeilidAPI, veilid_core::VeilidAPIError> {
    let mut api_lock = VEILID_API.lock().await;
    api_lock
        .take()
        .ok_or(veilid_core::VeilidAPIError::NotInitialized)
}

/////////////////////////////////////////
// FFI Helpers

// Declare external routine to release ffi strings
define_string_destructor!(free_string);

// Utility types for async API results
type APIResult<T> = Result<T, veilid_core::VeilidAPIError>;
const APIRESULT_VOID: APIResult<()> = APIResult::Ok(());

// Stream abort macro for simplified error handling
macro_rules! check_err_json {
    ($stream:expr, $ex:expr) => {
        match $ex {
            Ok(v) => v,
            Err(e) => {
                $stream.abort_json(e);
                return;
            }
        }
    };
}

/////////////////////////////////////////
// Initializer
#[no_mangle]
pub extern "C" fn initialize_veilid_flutter(dart_post_c_object_ptr: ffi::DartPostCObjectFnType) {
    unsafe {
        store_dart_post_cobject(dart_post_c_object_ptr);
    }

    use std::sync::Once;
    static INIT_BACKTRACE: Once = Once::new();
    INIT_BACKTRACE.call_once(move || {
        std::env::set_var("RUST_BACKTRACE", "1");
        std::panic::set_hook(Box::new(move |panic_info| {
            let (file, line) = if let Some(loc) = panic_info.location() {
                (loc.file(), loc.line())
            } else {
                ("<unknown>", 0)
            };
            error!("### Rust `panic!` hit at file '{}', line {}", file, line);
            if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                error!("panic payload: {:?}", s);
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                error!("panic payload: {:?}", s);
            } else if let Some(a) = panic_info.payload().downcast_ref::<std::fmt::Arguments>() {
                error!("panic payload: {:?}", a);
            } else {
                error!("no panic payload");
            }
            error!("  Complete stack trace:\n{:?}", backtrace::Backtrace::new());

            // And stop the process, no recovery is going to be possible here
            std::process::abort();
        }));
    });
}

//////////////////////////////////////////////////////////////////////////////////
/// C-compatible FFI Functions

#[no_mangle]
pub extern "C" fn startup_veilid_core(port: i64, config: FfiStr) {
    let config = config.into_opt_string();
    let stream = DartIsolateStream::new(port);
    async_std::task::spawn(async move {
        let config_json = match config {
            Some(v) => v,
            None => {
                stream.abort_json(veilid_core::VeilidAPIError::MissingArgument {
                    context: "startup_veilid_core".to_owned(),
                    argument: "config".to_owned(),
                });
                return;
            }
        };

        let mut api_lock = VEILID_API.lock().await;
        if api_lock.is_some() {
            stream.abort_json(veilid_core::VeilidAPIError::AlreadyInitialized);
            return;
        }

        let sink = stream.clone();
        let update_callback = Arc::new(move |update: veilid_core::VeilidUpdate| {
            let sink = sink.clone();
            match update {
                veilid_core::VeilidUpdate::Shutdown => {
                    sink.close();
                }
                _ => {
                    sink.item_json(update);
                }
            }
        });

        let res = veilid_core::api_startup_json(update_callback, config_json).await;
        let veilid_api = check_err_json!(stream, res);
        *api_lock = Some(veilid_api);
    });
}

#[no_mangle]
pub extern "C" fn get_veilid_state(port: i64) {
    DartIsolateWrapper::new(port).spawn_result_json(async move {
        let veilid_api = get_veilid_api().await?;
        let core_state = veilid_api.get_state().await?;
        APIResult::Ok(core_state)
    });
}

#[no_mangle]
pub extern "C" fn change_api_log_level(port: i64, log_level: FfiStr) {
    let log_level = log_level.into_opt_string();
    DartIsolateWrapper::new(port).spawn_result_json(async move {
        let log_level: veilid_core::VeilidConfigLogLevel =
            veilid_core::deserialize_opt_json(log_level)?;
        let veilid_api = get_veilid_api().await?;
        veilid_api.change_api_log_level(log_level).await;
        APIRESULT_VOID
    });
}

#[no_mangle]
pub extern "C" fn shutdown_veilid_core(port: i64) {
    DartIsolateWrapper::new(port).spawn_result_json(async move {
        let veilid_api = take_veilid_api().await?;
        veilid_api.shutdown().await;
        APIRESULT_VOID
    });
}

#[no_mangle]
pub extern "C" fn debug(port: i64, command: FfiStr) {
    let command = command.into_opt_string().unwrap_or_default();
    DartIsolateWrapper::new(port).spawn_result(async move {
        let veilid_api = get_veilid_api().await?;
        let out = veilid_api.debug(command).await?;
        APIResult::Ok(out)
    });
}

#[no_mangle]
pub extern "C" fn veilid_version_string() -> *mut c_char {
    veilid_core::veilid_version_string().into_ffi_value()
}

#[repr(C)]
pub struct VeilidVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[no_mangle]
pub extern "C" fn veilid_version() -> VeilidVersion {
    let (major, minor, patch) = veilid_core::veilid_version();
    VeilidVersion {
        major,
        minor,
        patch,
    }
}

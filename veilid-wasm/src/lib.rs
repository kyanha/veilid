// wasm-bindgen and clippy don't play well together yet
#![allow(clippy::all)]
#![cfg(target_arch = "wasm32")]
#![no_std]

extern crate alloc;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::*;
use core::any::{Any, TypeId};
use core::cell::RefCell;
use futures_util::FutureExt;
use js_sys::*;
use lazy_static::*;
use send_wrapper::*;
use serde::*;
use tracing::*;
use tracing_subscriber::prelude::*;
use tracing_subscriber::*;
use tracing_wasm::{WASMLayerConfigBuilder, *};
use veilid_core::xx::*;
use veilid_core::*;
use wasm_bindgen_futures::*;

// Allocator
extern crate wee_alloc;
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static SETUP_ONCE: Once = Once::new();
pub fn setup() -> () {
    SETUP_ONCE.call_once(|| {});
}

// Log filtering
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

// API Singleton
lazy_static! {
    static ref VEILID_API: SendWrapper<RefCell<Option<veilid_core::VeilidAPI>>> =
        SendWrapper::new(RefCell::new(None));
}

fn get_veilid_api() -> Result<veilid_core::VeilidAPI, veilid_core::VeilidAPIError> {
    (*VEILID_API)
        .borrow()
        .clone()
        .ok_or(veilid_core::VeilidAPIError::NotInitialized)
}

fn take_veilid_api() -> Result<veilid_core::VeilidAPI, veilid_core::VeilidAPIError> {
    (**VEILID_API)
        .take()
        .ok_or(veilid_core::VeilidAPIError::NotInitialized)
}

// JSON Marshalling
pub fn serialize_json<T: Serialize>(val: T) -> String {
    serde_json::to_string(&val).expect("failed to serialize json value")
}

pub fn deserialize_json<T: de::DeserializeOwned>(
    arg: &str,
) -> Result<T, veilid_core::VeilidAPIError> {
    serde_json::from_str(arg).map_err(|e| veilid_core::VeilidAPIError::ParseError {
        message: e.to_string(),
        value: String::new(),
    })
}

pub fn to_json<T: Serialize>(val: T) -> JsValue {
    JsValue::from_str(&serialize_json(val))
}

pub fn from_json<T: de::DeserializeOwned>(val: JsValue) -> Result<T, veilid_core::VeilidAPIError> {
    let s = val
        .as_string()
        .ok_or_else(|| veilid_core::VeilidAPIError::ParseError {
            message: "Value is not String".to_owned(),
            value: String::new(),
        })?;
    deserialize_json(&s)
}

// Utility types for async API results
type APIResult<T> = Result<T, veilid_core::VeilidAPIError>;
const APIRESULT_UNDEFINED: APIResult<()> = APIResult::Ok(());

pub fn wrap_api_future<F, T>(future: F) -> Promise
where
    F: Future<Output = APIResult<T>> + 'static,
    T: Serialize + 'static,
{
    future_to_promise(future.map(|res| {
        res.map(|v| {
            if TypeId::of::<()>() == v.type_id() {
                JsValue::UNDEFINED
            } else {
                to_json(v)
            }
        })
        .map_err(|e| to_json(e))
    }))
}

/////////////////////////////////////////
// WASM-specific cofnig

#[derive(Debug, Deserialize, Serialize)]
pub struct VeilidWASMConfigLoggingPerformance {
    pub enabled: bool,
    pub level: veilid_core::VeilidLogLevel,
    pub logs_in_timings: bool,
    pub logs_in_console: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VeilidWASMConfigLogging {
    pub performance: VeilidWASMConfigLoggingPerformance,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VeilidWASMConfig {
    pub logging: VeilidWASMConfigLogging,
}

// WASM Bindings

#[wasm_bindgen()]
pub fn initialize_veilid_wasm() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen()]
pub fn configure_veilid_platform(platform_config: String) {
    let platform_config: VeilidWASMConfig = veilid_core::deserialize_json(&platform_config)
        .expect("failed to deserialize plaform config json");

    // Set up subscriber and layers
    let mut ignore_list = Vec::<String>::new();
    for ig in veilid_core::DEFAULT_LOG_IGNORE_LIST {
        ignore_list.push(ig.to_owned());
    }

    let subscriber = Registry::default();

    // Performance logger
    let subscriber = subscriber.with(if platform_config.logging.performance.enabled {
        let performance_max_log_level =
            platform_config.logging.performance.level.to_tracing_level();

        let ignore_list = ignore_list.clone();
        Some(
            WASMLayer::new(
                WASMLayerConfigBuilder::new()
                    .set_report_logs_in_timings(platform_config.logging.performance.logs_in_timings)
                    .set_console_config(if platform_config.logging.performance.logs_in_console {
                        ConsoleConfig::ReportWithConsoleColor
                    } else {
                        ConsoleConfig::NoReporting
                    })
                    .set_max_level(performance_max_log_level)
                    .build(),
            )
            .with_filter(filter::FilterFn::new(move |metadata| {
                logfilter(metadata, &ignore_list)
            })),
        )
    } else {
        None
    });

    // API logger (always add layer, startup will init this if it is enabled in settings)
    let subscriber = subscriber.with(veilid_core::ApiTracingLayer::get());

    subscriber
        .try_init()
        .map_err(|e| format!("failed to initialize logging: {}", e))
        .expect("failed to initalize WASM platform");
}

#[wasm_bindgen()]
pub fn startup_veilid_core(update_callback: Function, json_config: String) -> Promise {
    wrap_api_future(async move {
        let update_callback = Arc::new(move |update: VeilidUpdate| {
            let _ret =
                match Function::call1(&update_callback, &JsValue::UNDEFINED, &to_json(update)) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("calling update callback failed: {:?}", e);
                        return;
                    }
                };
        });

        if VEILID_API.borrow().is_some() {
            return Err(veilid_core::VeilidAPIError::AlreadyInitialized);
        }

        let veilid_api = veilid_core::api_startup_json(update_callback, json_config).await?;
        VEILID_API.replace(Some(veilid_api));
        APIRESULT_UNDEFINED
    })
}

#[wasm_bindgen()]
pub fn get_veilid_state() -> Promise {
    wrap_api_future(async move {
        let veilid_api = get_veilid_api()?;
        let core_state = veilid_api.get_state().await?;
        Ok(core_state)
    })
}

#[wasm_bindgen()]
pub fn change_api_log_level(log_level: String) -> Promise {
    wrap_api_future(async move {
        let log_level: veilid_core::VeilidConfigLogLevel = deserialize_json(&log_level)?;
        //let veilid_api = get_veilid_api()?;
        //veilid_api.change_api_log_level(log_level).await;
        veilid_core::ApiTracingLayer::change_api_log_level(log_level.to_veilid_log_level());
        APIRESULT_UNDEFINED
    })
}

#[wasm_bindgen()]
pub fn shutdown_veilid_core() -> Promise {
    wrap_api_future(async move {
        let veilid_api = take_veilid_api()?;
        veilid_api.shutdown().await;
        APIRESULT_UNDEFINED
    })
}

#[wasm_bindgen()]
pub fn debug(command: String) -> Promise {
    wrap_api_future(async move {
        let veilid_api = get_veilid_api()?;
        let out = veilid_api.debug(command).await?;
        Ok(out)
    })
}

#[wasm_bindgen()]
pub fn veilid_version_string() -> String {
    veilid_core::veilid_version_string()
}

#[derive(Serialize)]
pub struct VeilidVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[wasm_bindgen()]
pub fn veilid_version() -> JsValue {
    let (major, minor, patch) = veilid_core::veilid_version();
    let vv = VeilidVersion {
        major,
        minor,
        patch,
    };
    JsValue::from_serde(&vv).unwrap()
}

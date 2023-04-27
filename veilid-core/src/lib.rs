#![deny(clippy::all)]
#![deny(unused_must_use)]
#![recursion_limit = "256"]

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        #[cfg(any(feature = "rt-async-std", feature = "rt-tokio"))]
        compile_error!("features \"rt-async-std\" and \"rt-tokio\" can not be specified for WASM");
    } else {
        #[cfg(all(feature = "rt-async-std", feature = "rt-tokio"))]
        compile_error!(
            "feature \"rt-async-std\" and feature \"rt-tokio\" cannot be enabled at the same time"
        );
        #[cfg(not(any(feature = "rt-async-std", feature = "rt-tokio")))]
        compile_error!("exactly one of feature \"rt-async-std\" or feature \"rt-tokio\" must be specified");
    }
}

#[macro_use]
extern crate alloc;

mod api_tracing_layer;
mod attachment_manager;
mod core_context;
mod crypto;
mod intf;
mod network_manager;
mod receipt_manager;
mod routing_table;
mod rpc_processor;
mod storage_manager;
mod veilid_api;
mod veilid_config;
mod veilid_layer_filter;

pub use self::api_tracing_layer::ApiTracingLayer;
pub use self::core_context::{api_startup, api_startup_json, UpdateCallback};
pub use self::veilid_api::*;
pub use self::veilid_config::*;
pub use self::veilid_layer_filter::*;
pub use veilid_tools as tools;

use enumset::*;
use rkyv::{
    bytecheck, bytecheck::CheckBytes, de::deserializers::SharedDeserializeMap, with::Skip,
    Archive as RkyvArchive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize,
};
type RkyvSerializer = rkyv::ser::serializers::CompositeSerializer<
    rkyv::ser::serializers::AlignedSerializer<rkyv::AlignedVec>,
    rkyv::ser::serializers::FallbackScratch<
        rkyv::ser::serializers::HeapScratch<1024>,
        rkyv::ser::serializers::AllocScratch,
    >,
    rkyv::ser::serializers::SharedSerializeMap,
>;
type RkyvDefaultValidator<'t> = rkyv::validation::validators::DefaultValidator<'t>;
use serde::*;

pub mod veilid_capnp {
    include!(concat!(env!("OUT_DIR"), "/proto/veilid_capnp.rs"));
}

pub mod tests;

pub fn veilid_version_string() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
pub fn veilid_version() -> (u32, u32, u32) {
    (
        u32::from_str(env!("CARGO_PKG_VERSION_MAJOR")).unwrap(),
        u32::from_str(env!("CARGO_PKG_VERSION_MINOR")).unwrap(),
        u32::from_str(env!("CARGO_PKG_VERSION_PATCH")).unwrap(),
    )
}

#[cfg(target_os = "android")]
pub use intf::android::veilid_core_setup_android;

pub static DEFAULT_LOG_IGNORE_LIST: [&str; 21] = [
    "mio",
    "h2",
    "hyper",
    "tower",
    "tonic",
    "tokio",
    "runtime",
    "tokio_util",
    "want",
    "serial_test",
    "async_std",
    "async_io",
    "polling",
    "rustls",
    "async_tungstenite",
    "tungstenite",
    "netlink_proto",
    "netlink_sys",
    "trust_dns_resolver",
    "trust_dns_proto",
    "attohttpc",
];

use veilid_tools::*;

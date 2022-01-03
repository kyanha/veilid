#![deny(clippy::all)]
#![deny(unused_must_use)]
#![cfg_attr(target_arch = "wasm32", no_std)]

#[macro_use]
extern crate alloc;

mod attachment_manager;
mod callback_state_machine;
mod connection_manager;
mod connection_table;
mod dht;
mod intf;
mod lease_manager;
mod network_manager;
mod receipt_manager;
mod routing_table;
mod rpc_processor;
mod veilid_api;
mod veilid_config;
mod veilid_core;
mod veilid_rng;

#[macro_use]
pub mod xx;

pub use self::attachment_manager::AttachmentState;
pub use self::veilid_api::*;
pub use self::veilid_config::*;
pub use self::veilid_core::{VeilidCore, VeilidCoreSetup, VeilidState, VeilidStateChange};

pub mod veilid_capnp {
    include!(concat!(env!("OUT_DIR"), "/proto/veilid_capnp.rs"));
}

pub mod tests;

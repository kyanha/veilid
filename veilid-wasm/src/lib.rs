#![cfg(target_arch = "wasm32")]
#![no_std]

#[macro_use]
extern crate alloc;

pub use log::*;
pub use wasm_bindgen::prelude::*;
pub use wasm_bindgen::JsCast;

pub use alloc::boxed::Box;
pub use alloc::string::String;
pub use alloc::sync::Arc;
pub use alloc::vec::Vec;
pub use core::convert::TryFrom;
pub use js_sys::*;
pub use js_veilid_core::*;
pub use utils::*;
pub use veilid_core::dht::key::*;
pub use veilid_core::xx::*;
pub use veilid_core::*;
pub use wasm_logger::*;

mod js_veilid_core;
mod utils;

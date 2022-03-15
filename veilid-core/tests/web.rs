//! Test suite for the Web and headless browsers.
#![cfg(target_arch = "wasm32")]

use veilid_core::tests::common::*;
use veilid_core::xx::*;
use veilid_core::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

extern crate wee_alloc;
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static SETUP_ONCE: Once = Once::new();
pub fn setup() -> () {
    SETUP_ONCE.call_once(|| {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::new(Level::Trace));
    });
}

#[wasm_bindgen_test]
async fn run_test_dht_key() {
    setup();

    test_dht_key::test_all().await;
}

#[wasm_bindgen_test]
async fn run_test_host_interface() {
    setup();

    test_host_interface::test_all().await;
}

#[wasm_bindgen_test]
async fn run_test_veilid_core() {
    setup();

    test_veilid_core::test_all().await;
}

#[wasm_bindgen_test]
async fn run_test_config() {
    setup();

    test_veilid_config::test_all().await;
}

#[wasm_bindgen_test]
async fn run_test_connection_table() {
    setup();

    test_connection_table::test_all().await;
}

#[wasm_bindgen_test]
async fn run_test_table_store() {
    setup();

    test_table_store::test_all().await;
}

#[wasm_bindgen_test]
async fn run_test_crypto() {
    setup();

    test_crypto::test_all().await;
}

#[wasm_bindgen_test]
async fn run_test_envelope_receipt() {
    setup();

    test_envelope_receipt::test_all().await;
}

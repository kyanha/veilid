//! Test suite for Native
#![cfg(not(target_arch = "wasm32"))]
use crate::crypto::tests::*;
use crate::network_manager::tests::*;
use crate::routing_table;
use crate::table_store::tests::*;
use crate::tests::common::*;
use crate::veilid_api;
use crate::*;

///////////////////////////////////////////////////////////////////////////

#[allow(dead_code)]
pub async fn run_all_tests() {
    // iOS and Android tests also run these.
    info!("TEST: test_host_interface");
    test_host_interface::test_all().await;
    info!("TEST: test_types");
    test_types::test_all().await;
    info!("TEST: test_veilid_core");
    test_veilid_core::test_all().await;
    info!("TEST: test_veilid_config");
    test_veilid_config::test_all().await;
    info!("TEST: test_connection_table");
    test_connection_table::test_all().await;
    info!("TEST: test_signed_node_info");
    test_signed_node_info::test_all().await;
    info!("TEST: test_table_store");
    test_table_store::test_all().await;
    info!("TEST: test_protected_store");
    test_protected_store::test_all().await;
    info!("TEST: test_crypto");
    test_crypto::test_all().await;
    info!("TEST: test_envelope_receipt");
    test_envelope_receipt::test_all().await;
    info!("TEST: veilid_api::tests::test_serialize_json");
    veilid_api::tests::test_serialize_json::test_all().await;
    info!("TEST: veilid_api::tests::test_serialize_rkyv");
    veilid_api::tests::test_serialize_rkyv::test_all().await;
    info!("TEST: routing_table::test_serialize_routing_table");
    routing_table::tests::test_serialize_routing_table::test_all().await;

    info!("Finished unit tests");
}

#[allow(dead_code)]
#[cfg(feature = "rt-tokio")]
pub fn block_on<F: Future<Output = T>, T>(f: F) -> T {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(f)
}

#[cfg(feature = "rt-async-std")]
#[allow(dead_code)]
pub fn block_on<F: Future<Output = T>, T>(f: F) -> T {
    async_std::task::block_on(f)
}

///////////////////////////////////////////////////////////////////////////
cfg_if! {
    if #[cfg(test)] {
        use serial_test::serial;
        use std::sync::Once;
        use paste::paste;

        macro_rules! run_test {
            // Nearly all test runner code is cookie cutter, and copy-pasting makes it too easy to make a typo.

            // Pass in a module and test module, and we'll run its `test_all`.
            ($parent_module:ident, $test_module:ident) => {
                paste! {
                    #[test]
                    #[serial]
                    fn [<run_ $parent_module _ $test_module>]() {
                        setup();
                        block_on(async {
                            $parent_module::tests::$test_module::test_all().await;
                        })
                    }
                }
            };

            // Pass in a test module name, and we'll run its `test_all`.
            ($test_module:ident) => {
                paste! {
                    #[test]
                    #[serial]
                    fn [<run_ $test_module>]() {
                        setup();
                        block_on(async {
                            $test_module::test_all().await;
                        })
                    }
                }
            };
        }

        static SETUP_ONCE: Once = Once::new();

        pub fn setup() {
            SETUP_ONCE.call_once(|| {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        use tracing_subscriber::{filter, fmt, prelude::*};
                        let mut filters = filter::Targets::new().with_default(filter::LevelFilter::TRACE);
                        for ig in DEFAULT_LOG_IGNORE_LIST {
                            filters = filters.with_target(ig, filter::LevelFilter::OFF);
                        }
                        let fmt_layer = fmt::layer();
                        tracing_subscriber::registry()
                            .with(fmt_layer)
                            .with(filters)
                            .init();
                    }
                }
            });
        }

        run_test!(test_host_interface);

        run_test!(test_types);

        run_test!(test_veilid_core);

        run_test!(test_veilid_config);

        run_test!(test_connection_table);

        run_test!(test_signed_node_info);

        run_test!(test_table_store);

        run_test!(test_protected_store);

        run_test!(test_crypto);

        run_test!(test_envelope_receipt);

        run_test!(veilid_api, test_serialize_json);

        run_test!(veilid_api, test_serialize_rkyv);

        run_test!(routing_table, test_serialize_routing_table);
    }
}

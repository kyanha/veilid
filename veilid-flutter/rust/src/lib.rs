mod api;
mod bridge_generated;

use cfg_if::*;

xxx make this work

#[cfg(all(target_os = "android", feature = "android_tests"))]
use jni::{objects::JClass, objects::JObject, JNIEnv};

#[cfg(all(target_os = "android", feature = "android_tests"))]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_com_veilid_veilidcore_veilidcore_1android_1tests_MainActivity_run_1tests(
    env: JNIEnv,
    _class: JClass,
    ctx: JObject,
) {
    crate::intf::utils::android::veilid_core_setup_android(env, ctx, "veilid_core", Level::Trace);
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn run_veilid_core_tests(app_name: c_str) {
    let log_path: std::path::PathBuf = [
        std::env::var("HOME").unwrap().as_str(),
        "Documents",
        "veilid-core.log",
    ]
    .iter()
    .collect();
    crate::intf::utils::setup::veilid_core_setup(
        "veilid-core",
        Some(Level::Trace),
        Some((Level::Trace, log_path.as_path())),
    );
}

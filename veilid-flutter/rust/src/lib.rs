use cfg_if::*;

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        mod dart_ffi;
        mod dart_isolate_wrapper;
        mod dart_serialize;
    } else {
        mod wasm;
    }
}

#[cfg(target_os = "android")]
use jni::{objects::JClass, objects::JObject, JNIEnv};

#[cfg(target_os = "android")]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_com_veilid_veilid_VeilidPlugin_init_1android(
    env: JNIEnv,
    _class: JClass,
    ctx: JObject,
) {
    veilid_core::veilid_core_setup_android_no_log(env, ctx);
}

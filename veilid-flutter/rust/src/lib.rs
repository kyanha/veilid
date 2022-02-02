mod api;
mod bridge_generated;

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
    crate::intf::utils::android::veilid_core_setup_android_no_log(env, ctx);
}

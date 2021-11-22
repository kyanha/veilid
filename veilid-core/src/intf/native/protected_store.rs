use cfg_if::*;
use keyring::{Keyring, KeyringError};

fn keyring_name(namespace: &str) -> String {
    if namespace.len() == 0 {
        "veilid".to_owned()
    } else {
        format!("veilid_{}", namespace)
    }
}

fn get_keyring<'a>(krname: &'a str, key: &'a str) -> Keyring<'a> {
    cfg_if! {
        if #[cfg(target_os = "android")] {
            let agopt = super::utils::android::ANDROID_GLOBALS.lock();
            let ag = agopt.as_ref().unwrap();
            let vm = ag.vm.attach_current_thread().unwrap().get_java_vm().unwrap(); // cmon jni, no clone for javavm
            let ctx = ag.ctx.clone();
            Keyring::new("veilid", krname, key, (vm, ctx))
        } else {
            Keyring::new("veilid", krname, key)
        }
    }
}

pub async fn save_user_secret_string(
    namespace: &str,
    key: &str,
    value: &str,
) -> Result<bool, String> {
    let krname = keyring_name(namespace);
    let kr = get_keyring(krname.as_str(), key);
    let existed = kr.get_password().is_ok();
    let _ = kr
        .set_password(value)
        .map_err(|e| format!("Failed to save user secret: {}", e).to_owned())?;
    Ok(existed)
}

pub async fn load_user_secret_string(namespace: &str, key: &str) -> Result<Option<String>, String> {
    let krname = keyring_name(namespace);
    let kr = get_keyring(krname.as_str(), key);
    match kr.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(KeyringError::NoPasswordFound) => Ok(None),
        Err(e) => Err(format!("Failed to load user secret: {}", e).to_owned()),
    }
}

pub async fn remove_user_secret_string(namespace: &str, key: &str) -> Result<bool, String> {
    let krname = keyring_name(namespace);
    let kr = get_keyring(krname.as_str(), key);
    match kr.delete_password() {
        Ok(_) => Ok(true),
        Err(KeyringError::NoPasswordFound) => Ok(false),
        Err(e) => Err(format!("Failed to remove user secret: {}", e).to_owned()),
    }
}

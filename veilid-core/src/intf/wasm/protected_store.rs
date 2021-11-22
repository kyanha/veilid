use super::utils;
use crate::xx::*;
use js_sys::*;
use wasm_bindgen_futures::*;
use web_sys::*;
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_name = setPassword, js_namespace = ["global", "wasmhost", "keytar"])]
    fn keytar_setPassword(service: &str, account: &str, password: &str)
        -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch, js_name = getPassword, js_namespace = ["global", "wasmhost", "keytar"])]
    fn keytar_getPassword(service: &str, account: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(catch, js_name = deletePassword, js_namespace = ["global", "wasmhost", "keytar"])]
    fn keytar_deletePassword(service: &str, account: &str) -> Result<Promise, JsValue>;
}

fn keyring_name(namespace: &str) -> String {
    if namespace.len() == 0 {
        "veilid".to_owned()
    } else {
        format!("veilid_{}", namespace)
    }
}

fn browser_key_name(namespace: &str, key: &str) -> String {
    if namespace.len() == 0 {
        format!("__veilid_secret_{}", key)
    } else {
        format!("__veilid_{}_secret_{}", namespace, key)
    }
}

pub async fn save_user_secret_string(
    namespace: &str,
    key: &str,
    value: &str,
) -> Result<bool, String> {
    if utils::is_nodejs() {
        let prev = match JsFuture::from(
            keytar_getPassword(keyring_name(namespace).as_str(), key)
                .map_err(|_| "exception thrown".to_owned())?,
        )
        .await
        {
            Ok(v) => v.is_truthy(),
            Err(_) => false,
        };

        match JsFuture::from(
            keytar_setPassword(keyring_name(namespace).as_str(), key, value)
                .map_err(|_| "exception thrown".to_owned())?,
        )
        .await
        {
            Ok(_) => {}
            Err(_) => return Err("Failed to set password".to_owned()),
        }

        Ok(prev)
    } else if utils::is_browser() {
        let win = match window() {
            Some(w) => w,
            None => {
                return Err("failed to get window".to_owned());
            }
        };

        let ls = match win
            .local_storage()
            .map_err(|_| "exception getting local storage".to_owned())?
        {
            Some(l) => l,
            None => {
                return Err("failed to get local storage".to_owned());
            }
        };

        let vkey = browser_key_name(namespace, key);

        let prev = match ls
            .get_item(&vkey)
            .map_err(|_| "exception_thrown".to_owned())?
        {
            Some(_) => true,
            None => false,
        };

        ls.set_item(&vkey, value)
            .map_err(|_| "exception_thrown".to_owned())?;

        Ok(prev)
    } else {
        Err("unimplemented".to_owned())
    }
}

pub async fn load_user_secret_string(namespace: &str, key: &str) -> Result<Option<String>, String> {
    if utils::is_nodejs() {
        let prev = match JsFuture::from(
            keytar_getPassword(keyring_name(namespace).as_str(), key)
                .map_err(|_| "exception thrown".to_owned())?,
        )
        .await
        {
            Ok(p) => p,
            Err(_) => JsValue::UNDEFINED,
        };

        if prev.is_undefined() || prev.is_null() {
            return Ok(None);
        }

        Ok(prev.as_string())
    } else if utils::is_browser() {
        let win = match window() {
            Some(w) => w,
            None => {
                return Err("failed to get window".to_owned());
            }
        };

        let ls = match win
            .local_storage()
            .map_err(|_| "exception getting local storage".to_owned())?
        {
            Some(l) => l,
            None => {
                return Err("failed to get local storage".to_owned());
            }
        };

        let vkey = browser_key_name(namespace, key);

        ls.get_item(&vkey)
            .map_err(|_| "exception_thrown".to_owned())
    } else {
        Err("unimplemented".to_owned())
    }
}

pub async fn remove_user_secret_string(namespace: &str, key: &str) -> Result<bool, String> {
    if utils::is_nodejs() {
        match JsFuture::from(
            keytar_deletePassword("veilid", key).map_err(|_| "exception thrown".to_owned())?,
        )
        .await
        {
            Ok(v) => Ok(v.is_truthy()),
            Err(_) => Err("Failed to delete".to_owned()),
        }
    } else if utils::is_browser() {
        let win = match window() {
            Some(w) => w,
            None => {
                return Err("failed to get window".to_owned());
            }
        };

        let ls = match win
            .local_storage()
            .map_err(|_| "exception getting local storage".to_owned())?
        {
            Some(l) => l,
            None => {
                return Err("failed to get local storage".to_owned());
            }
        };

        let vkey = browser_key_name(namespace, key);

        match ls
            .get_item(&vkey)
            .map_err(|_| "exception_thrown".to_owned())?
        {
            Some(_) => {
                ls.delete(&vkey)
                    .map_err(|_| "exception_thrown".to_owned())?;
                Ok(true)
            }
            None => Ok(false),
        }
    } else {
        Err("unimplemented".to_owned())
    }
}

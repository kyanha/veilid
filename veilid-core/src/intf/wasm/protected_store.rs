use super::utils;
use crate::xx::*;
use crate::*;
use data_encoding::BASE64URL_NOPAD;
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


#[derive(Clone)]
pub struct ProtectedStore {
    config: VeilidConfig,
}

impl ProtectedStore {

    pub fn new(config: VeilidConfig) -> Self {
        Self {
            config,
        }
    }

    pub async fn delete_all(&self) -> EyreResult<()> {
        // Delete all known keys
        if self.remove_user_secret_string("node_id").await? {
            debug!("deleted protected_store key 'node_id'");
        }
        if self.remove_user_secret_string("node_id_secret").await? {
            debug!("deleted protected_store key 'node_id_secret'");
        }
        if self.remove_user_secret_string("_test_key").await? {
            debug!("deleted protected_store key '_test_key'");
        }
        Ok(())
    }

    pub async fn init(&self) -> EyreResult<()> {
        Ok(())
    }

    pub async fn terminate(&self) {}

    fn keyring_name(&self) -> String {
        let c = self.config.get();
        if c.namespace.is_empty() {
            "veilid_protected_store".to_owned()
        } else {
            format!("veilid_protected_store_{}", c.namespace)
        }
    }

    fn browser_key_name(&self, key: &str) -> String {
        let c = self.config.get();
        if c.namespace.is_empty() {
            format!("__veilid_protected_store_{}", key)
        } else {
            format!("__veilid_protected_store_{}_{}", c.namespace, key)
        }
    }

    pub async fn save_user_secret_string(&self, key: &str, value: &str) -> EyreResult<bool> {
        if utils::is_nodejs() {
            let prev = match JsFuture::from(
                keytar_getPassword(self.keyring_name().as_str(), key)
                    .wrap_err("exception thrown")?,
            )
            .await
            {
                Ok(v) => v.is_truthy(),
                Err(_) => false,
            };

            match JsFuture::from(
                keytar_setPassword(self.keyring_name().as_str(), key, value)
                    .wrap_err("exception thrown")?,
            )
            .await
            {
                Ok(_) => {}
                Err(_) => bail!("Failed to set password"),
            }

            Ok(prev)
        } else if utils::is_browser() {
            let win = match window() {
                Some(w) => w,
                None => {
                    bail!("failed to get window");
                }
            };

            let ls = match win
                .local_storage()
                .wrap_err("exception getting local storage")?
            {
                Some(l) => l,
                None => {
                    bail!("failed to get local storage");
                }
            };

            let vkey = self.browser_key_name(key);

            let prev = match ls
                .get_item(&vkey)
                .wrap_err("exception_thrown")?
            {
                Some(_) => true,
                None => false,
            };

            ls.set_item(&vkey, value)
                .wrap_err("exception_thrown")?;

            Ok(prev)
        } else {
            unimplemented!()
        }
    }

    pub async fn load_user_secret_string(&self, key: &str) -> EyreResult<Option<String>> {
        if utils::is_nodejs() {
            let prev = match JsFuture::from(
                keytar_getPassword(self.keyring_name().as_str(), key)
                    .wrap_err("exception thrown")?,
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
                    bail!("failed to get window");
                }
            };

            let ls = match win
                .local_storage()
                .wrap_err("exception getting local storage")?
            {
                Some(l) => l,
                None => {
                    bail!("failed to get local storage");
                }
            };

            let vkey = self.browser_key_name(key);

            ls.get_item(&vkey)
                .wrap_err("exception_thrown")
        } else {
            unimplemented!();
        }
    }

    pub async fn remove_user_secret_string(&self, key: &str) -> EyreResult<bool> {
        if utils::is_nodejs() {
            match JsFuture::from(
                keytar_deletePassword(self.keyring_name().as_str(), key).wrap_err("exception thrown")?,
            )
            .await
            {
                Ok(v) => Ok(v.is_truthy()),
                Err(_) => bail!("Failed to delete"),
            }
        } else if utils::is_browser() {
            let win = match window() {
                Some(w) => w,
                None => {
                    bail!("failed to get window");
                }
            };

            let ls = match win
                .local_storage()
                .wrap_err("exception getting local storage")?
            {
                Some(l) => l,
                None => {
                    bail!("failed to get local storage");
                }
            };

            let vkey = self.browser_key_name(key);

            match ls
                .get_item(&vkey)
                .wrap_err("exception_thrown")?
            {
                Some(_) => {
                    ls.delete(&vkey)
                        .wrap_err("exception_thrown")?;
                    Ok(true)
                }
                None => Ok(false),
            }
        } else {
            unimplemented!();
        }
    }
        
    pub async fn save_user_secret(&self, key: &str, value: &[u8]) -> EyreResult<bool> {
        let mut s = BASE64URL_NOPAD.encode(value);
        s.push('!');

        self.save_user_secret_string(key, s.as_str()).await
    }

    pub async fn load_user_secret(&self, key: &str) -> EyreResult<Option<Vec<u8>>> {
        let mut s = match self.load_user_secret_string(key).await? {
            Some(s) => s,
            None => {
                return Ok(None);
            }
        };

        if s.pop() != Some('!') {
            bail!("User secret is not a buffer");
        }

        let mut bytes = Vec::<u8>::new();
        let res = BASE64URL_NOPAD.decode_len(s.len());
        match res {
            Ok(l) => {
                bytes.resize(l, 0u8);
            }
            Err(_) => {
                bail!("Failed to decode");
            }
        }

        let res = BASE64URL_NOPAD.decode_mut(s.as_bytes(), &mut bytes);
        match res {
            Ok(_) => Ok(Some(bytes)),
            Err(_) => bail!("Failed to decode"),
        }
    }

    pub async fn remove_user_secret(&self, key: &str) -> EyreResult<bool> {
        self.remove_user_secret_string(key).await
    }
}
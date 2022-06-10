use crate::xx::*;
use crate::*;
use data_encoding::BASE64URL_NOPAD;
use keyring_manager::*;
use std::path::Path;
use std::result::Result;

pub struct ProtectedStoreInner {
    keyring_manager: Option<KeyringManager>,
}

#[derive(Clone)]
pub struct ProtectedStore {
    config: VeilidConfig,
    inner: Arc<Mutex<ProtectedStoreInner>>,
}

impl ProtectedStore {
    fn new_inner() -> ProtectedStoreInner {
        ProtectedStoreInner {
            keyring_manager: None,
        }
    }

    pub fn new(config: VeilidConfig) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(Self::new_inner())),
        }
    }

    #[instrument(level = "trace", skip(self), err)]
    pub async fn delete_all(&self) -> Result<(), String> {
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

    #[instrument(level = "debug", skip(self), err)]
    pub async fn init(&self) -> Result<(), String> {
        let delete = {
            let c = self.config.get();
            let mut inner = self.inner.lock();
            if !c.protected_store.always_use_insecure_storage {
                // Attempt to open the secure keyring
                cfg_if! {
                    if #[cfg(target_os = "android")] {
                        inner.keyring_manager = KeyringManager::new_secure(&c.program_name, intf::native::utils::android::get_android_globals()).ok();
                    } else {
                        inner.keyring_manager = KeyringManager::new_secure(&c.program_name).ok();
                    }
                }
            }
            if (c.protected_store.always_use_insecure_storage
                || c.protected_store.allow_insecure_fallback)
                && inner.keyring_manager.is_none()
            {
                let insecure_fallback_directory =
                    Path::new(&c.protected_store.insecure_fallback_directory);
                let insecure_keyring_file = insecure_fallback_directory.to_owned().join(format!(
                    "insecure_keyring{}",
                    if c.namespace.is_empty() {
                        "".to_owned()
                    } else {
                        format!("_{}", c.namespace)
                    }
                ));

                // Ensure permissions are correct
                ensure_file_private_owner(&insecure_keyring_file)?;

                // Open the insecure keyring
                inner.keyring_manager = Some(
                    KeyringManager::new_insecure(&c.program_name, &insecure_keyring_file)
                        .map_err(map_to_string)
                        .map_err(logthru_pstore!(error))?,
                );
            }
            if inner.keyring_manager.is_none() {
                return Err("Could not initialize the protected store.".to_owned());
            }
            c.protected_store.delete
        };

        if delete {
            self.delete_all().await?;
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn terminate(&self) {
        *self.inner.lock() = Self::new_inner();
    }

    fn service_name(&self) -> String {
        let c = self.config.get();
        if c.namespace.is_empty() {
            "veilid_protected_store".to_owned()
        } else {
            format!("veilid_protected_store_{}", c.namespace)
        }
    }

    #[instrument(level = "trace", skip(self, value), ret, err)]
    pub async fn save_user_secret_string(&self, key: &str, value: &str) -> Result<bool, String> {
        let inner = self.inner.lock();
        inner
            .keyring_manager
            .as_ref()
            .ok_or_else(|| "Protected store not initialized".to_owned())?
            .with_keyring(&self.service_name(), key, |kr| {
                let existed = kr.get_value().is_ok();
                kr.set_value(value)
                    .map_err(|e| format!("Failed to save user secret: {}", e))?;
                Ok(existed)
            })
            .map_err(map_to_string)
            .map_err(logthru_pstore!())
    }

    #[instrument(level = "trace", skip(self), err)]
    pub async fn load_user_secret_string(&self, key: &str) -> Result<Option<String>, String> {
        let inner = self.inner.lock();
        match inner
            .keyring_manager
            .as_ref()
            .ok_or_else(|| "Protected store not initialized".to_owned())?
            .with_keyring(&self.service_name(), key, |kr| kr.get_value())
            .map_err(logthru_pstore!())
        {
            Ok(v) => Ok(Some(v)),
            Err(KeyringError::NoPasswordFound) => Ok(None),
            Err(e) => Err(format!("Failed to load user secret: {}", e)),
        }
    }

    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn remove_user_secret_string(&self, key: &str) -> Result<bool, String> {
        let inner = self.inner.lock();
        match inner
            .keyring_manager
            .as_ref()
            .ok_or_else(|| "Protected store not initialized".to_owned())?
            .with_keyring(&self.service_name(), key, |kr| kr.delete_value())
            .map_err(logthru_pstore!())
        {
            Ok(_) => Ok(true),
            Err(KeyringError::NoPasswordFound) => Ok(false),
            Err(e) => Err(format!("Failed to remove user secret: {}", e)),
        }
    }

    #[instrument(level = "trace", skip(self, value), ret, err)]
    pub async fn save_user_secret(&self, key: &str, value: &[u8]) -> Result<bool, String> {
        let mut s = BASE64URL_NOPAD.encode(value);
        s.push('!');

        self.save_user_secret_string(key, s.as_str()).await
    }

    #[instrument(level = "trace", skip(self), err)]
    pub async fn load_user_secret(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let mut s = match self.load_user_secret_string(key).await? {
            Some(s) => s,
            None => {
                return Ok(None);
            }
        };

        if s.pop() != Some('!') {
            return Err("User secret is not a buffer".to_owned());
        }

        let mut bytes = Vec::<u8>::new();
        let res = BASE64URL_NOPAD.decode_len(s.len());
        match res {
            Ok(l) => {
                bytes.resize(l, 0u8);
            }
            Err(_) => {
                return Err("Failed to decode".to_owned());
            }
        }

        let res = BASE64URL_NOPAD.decode_mut(s.as_bytes(), &mut bytes);
        match res {
            Ok(_) => Ok(Some(bytes)),
            Err(_) => Err("Failed to decode".to_owned()),
        }
    }

    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn remove_user_secret(&self, key: &str) -> Result<bool, String> {
        self.remove_user_secret_string(key).await
    }
}

mod byte_array_types;
mod envelope;
mod receipt;
mod types;
mod value;

pub mod crypto_system;
pub mod tests;
pub mod vld0;

pub use byte_array_types::*;
pub use crypto_system::*;
pub use envelope::*;
pub use receipt::*;
pub use types::*;
pub use value::*;
pub use vld0::*;

use crate::*;
use core::convert::TryInto;
use hashlink::linked_hash_map::Entry;
use hashlink::LruCache;
use serde::{Deserialize, Serialize};

pub type CryptoSystemVersion = Arc<dyn CryptoSystem + Send + Sync>;
pub const VALID_CRYPTO_KINDS: [CryptoKind; 1] = [CRYPTO_KIND_VLD0];

pub const MIN_ENVELOPE_VERSION: u8 = 0u8;
pub const MAX_ENVELOPE_VERSION: u8 = 0u8;

const DH_CACHE_SIZE: usize = 4096;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
struct DHCacheKey {
    key: PublicKey,
    secret: SecretKey,
}

#[derive(Serialize, Deserialize)]
struct DHCacheValue {
    shared_secret: SharedSecret,
}
type DHCache = LruCache<DHCacheKey, DHCacheValue>;

fn cache_to_bytes(cache: &DHCache) -> Vec<u8> {
    let cnt: usize = cache.len();
    let mut out: Vec<u8> = Vec::with_capacity(cnt * (32 + 32 + 32));
    for e in cache.iter() {
        out.extend(&e.0.key.bytes);
        out.extend(&e.0.secret.bytes);
        out.extend(&e.1.shared_secret.bytes);
    }
    let mut rev: Vec<u8> = Vec::with_capacity(out.len());
    for d in out.chunks(32 + 32 + 32).rev() {
        rev.extend(d);
    }
    rev
}

fn bytes_to_cache(bytes: &[u8], cache: &mut DHCache) {
    for d in bytes.chunks(32 + 32 + 32) {
        let k = DHCacheKey {
            key: PublicKey::new(d[0..32].try_into().expect("asdf")),
            secret: SecretKey::new(d[32..64].try_into().expect("asdf")),
        };
        let v = DHCacheValue {
            shared_secret: SharedSecret::new(d[64..96].try_into().expect("asdf")),
        };
        cache.insert(k, v);
    }
}

struct CryptoInner {
    dh_cache: DHCache,
    flush_future: Option<SendPinBoxFuture<()>>,
    crypto_vld0: Option<Arc<dyn CryptoSystem + Send + Sync>>,
}

struct CryptoUnlockedInner {
    config: VeilidConfig,
    table_store: TableStore,
    protected_store: ProtectedStore,
}

/// Crypto factory implementation
#[derive(Clone)]
pub struct Crypto {
    unlocked_inner: Arc<CryptoUnlockedInner>,
    inner: Arc<Mutex<CryptoInner>>,
}

impl Crypto {
    fn new_inner() -> CryptoInner {
        CryptoInner {
            dh_cache: DHCache::new(DH_CACHE_SIZE),
            flush_future: None,
            crypto_vld0: None,
        }
    }

    pub fn new(
        config: VeilidConfig,
        table_store: TableStore,
        protected_store: ProtectedStore,
    ) -> Self {
        let out = Self {
            unlocked_inner: Arc::new(CryptoUnlockedInner {
                config,
                table_store,
                protected_store,
            }),
            inner: Arc::new(Mutex::new(Self::new_inner())),
        };

        out.inner.lock().crypto_vld0 = Some(Arc::new(vld0::CryptoSystemVLD0::new(out.clone())));

        out
    }

    pub async fn init(&self) -> EyreResult<()> {
        trace!("Crypto::init");
        let table_store = self.unlocked_inner.table_store.clone();

        // Init node id from config
        if let Err(e) = self
            .unlocked_inner
            .config
            .init_node_ids(self.clone(), self.unlocked_inner.protected_store.clone())
            .await
        {
            return Err(e).wrap_err("init node id failed");
        }

        // make local copy of node id for easy access
        let mut cache_validity_key: Vec<u8> = Vec::new();
        {
            let c = self.unlocked_inner.config.get();
            for ck in &VALID_CRYPTO_KINDS {
                cache_validity_key.append(
                    &mut c
                        .network
                        .routing_table
                        .node_ids
                        .get(ck)
                        .unwrap()
                        .node_id
                        .unwrap()
                        .bytes
                        .to_vec(),
                );
            }
        };

        // load caches if they are valid for this node id
        let mut db = table_store.open("crypto_caches", 1).await?;
        let caches_valid = match db.load(0, b"cache_validity_key")? {
            Some(v) => v == cache_validity_key,
            None => false,
        };
        if caches_valid {
            if let Some(b) = db.load(0, b"dh_cache")? {
                let mut inner = self.inner.lock();
                bytes_to_cache(&b, &mut inner.dh_cache);
            }
        } else {
            drop(db);
            table_store.delete("crypto_caches").await?;
            db = table_store.open("crypto_caches", 1).await?;
            db.store(0, b"cache_validity_key", &cache_validity_key)
                .await?;
        }

        // Schedule flushing
        let this = self.clone();
        let flush_future = interval(60000, move || {
            let this = this.clone();
            async move {
                if let Err(e) = this.flush().await {
                    warn!("flush failed: {}", e);
                }
            }
        });
        self.inner.lock().flush_future = Some(flush_future);

        Ok(())
    }

    pub async fn flush(&self) -> EyreResult<()> {
        //trace!("Crypto::flush");
        let cache_bytes = {
            let inner = self.inner.lock();
            cache_to_bytes(&inner.dh_cache)
        };

        let db = self
            .unlocked_inner
            .table_store
            .open("crypto_caches", 1)
            .await?;
        db.store(0, b"dh_cache", &cache_bytes).await?;
        Ok(())
    }

    pub async fn terminate(&self) {
        trace!("Crypto::terminate");
        let flush_future = self.inner.lock().flush_future.take();
        if let Some(f) = flush_future {
            f.await;
        }
        trace!("starting termination flush");
        match self.flush().await {
            Ok(_) => {
                trace!("finished termination flush");
            }
            Err(e) => {
                error!("failed termination flush: {}", e);
            }
        };
    }

    /// Factory method to get a specific crypto version
    pub fn get(&self, kind: CryptoKind) -> Option<CryptoSystemVersion> {
        let inner = self.inner.lock();
        match kind {
            CRYPTO_KIND_VLD0 => Some(inner.crypto_vld0.clone().unwrap()),
            _ => None,
        }
    }

    /// Signature set verification
    /// Returns the set of signature cryptokinds that validate and are supported
    /// If any cryptokinds are supported and do not validate, the whole operation
    /// returns an error
    pub fn verify_signatures<F, R>(
        &self,
        data: &[u8],
        signatures: &[TypedKeySignature],
        transform: F,
    ) -> Result<Vec<R>, VeilidAPIError>
    where
        F: Fn(&TypedKeySignature) -> R,
    {
        let mut out = Vec::<R>::with_capacity(signatures.len());
        for sig in signatures {
            if let Some(vcrypto) = self.get(sig.kind) {
                vcrypto.verify(&sig.key, data, &sig.signature)?;
                out.push(transform(sig));
            }
        }
        Ok(out)
    }

    /// Signature set generation
    /// Generates the set of signatures that are supported
    /// Any cryptokinds that are not supported are silently dropped
    pub fn generate_signatures<F, R>(
        &self,
        data: &[u8],
        keypairs: &[TypedKeyPair],
        transform: F,
    ) -> Result<Vec<R>, VeilidAPIError>
    where
        F: Fn(&TypedKeyPair, Signature) -> R,
    {
        let mut out = Vec::<R>::with_capacity(keypairs.len());
        for kp in keypairs {
            if let Some(vcrypto) = self.get(kp.kind) {
                let sig = vcrypto.sign(&kp.key, &kp.secret, data)?;
                out.push(transform(kp, sig))
            }
        }
        Ok(out)
    }

    // Internal utilities

    fn cached_dh_internal<T: CryptoSystem>(
        &self,
        vcrypto: &T,
        key: &PublicKey,
        secret: &SecretKey,
    ) -> Result<SharedSecret, VeilidAPIError> {
        Ok(
            match self.inner.lock().dh_cache.entry(DHCacheKey {
                key: *key,
                secret: *secret,
            }) {
                Entry::Occupied(e) => e.get().shared_secret,
                Entry::Vacant(e) => {
                    let shared_secret = vcrypto.compute_dh(key, secret)?;
                    e.insert(DHCacheValue { shared_secret });
                    shared_secret
                }
            },
        )
    }
}

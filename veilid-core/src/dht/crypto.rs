use super::key::*;
use crate::intf::*;
use crate::xx::*;
use crate::*;
use chacha20poly1305 as ch;
use chacha20poly1305::aead::{AeadInPlace, NewAead};
use core::convert::TryInto;
use curve25519_dalek as cd;
use ed25519_dalek as ed;
use serde::{Deserialize, Serialize};
use serde_big_array::*;
use uluru;
use x25519_dalek as xd;

pub type SharedSecret = [u8; 32];
pub type Nonce = [u8; 24];

const DH_CACHE_SIZE: usize = 1024;
pub const ENCRYPTION_OVERHEAD: usize = 16;

big_array! {
    BigArray;
    DH_CACHE_SIZE
}

type DHCache = uluru::LRUCache<DHCacheEntry, DH_CACHE_SIZE>;

#[derive(Serialize, Deserialize)]
struct DHCacheEntry {
    key: DHTKey,
    secret: DHTKeySecret,
    shared_secret: SharedSecret,
}

fn cache_to_bytes(cache: &DHCache) -> Vec<u8> {
    let cnt: usize = cache.len();
    let mut out: Vec<u8> = Vec::with_capacity(cnt * (32 + 32 + 32));
    for e in cache.iter() {
        out.extend(&e.key.bytes);
        out.extend(&e.secret.bytes);
        out.extend(&e.shared_secret);
    }
    let mut rev: Vec<u8> = Vec::with_capacity(out.len());
    for d in out.chunks(32 + 32 + 32).rev() {
        rev.extend(d);
    }
    rev
}

fn bytes_to_cache(bytes: &[u8], cache: &mut DHCache) {
    for d in bytes.chunks(32 + 32 + 32) {
        let e = DHCacheEntry {
            key: DHTKey::new(d[0..32].try_into().expect("asdf")),
            secret: DHTKeySecret::new(d[32..64].try_into().expect("asdf")),
            shared_secret: d[64..96].try_into().expect("asdf"),
        };
        cache.insert(e);
    }
}

struct CryptoInner {
    table_store: TableStore,
    node_id: DHTKey,
    node_id_secret: DHTKeySecret,
    dh_cache: DHCache,
    flush_future: Option<SystemPinBoxFuture<()>>,
}

#[derive(Clone)]
pub struct Crypto {
    config: VeilidConfig,
    inner: Arc<Mutex<CryptoInner>>,
}

impl Crypto {
    fn new_inner(table_store: TableStore) -> CryptoInner {
        CryptoInner {
            table_store,
            node_id: Default::default(),
            node_id_secret: Default::default(),
            dh_cache: DHCache::default(),
            flush_future: None,
        }
    }

    pub fn new(config: VeilidConfig, table_store: TableStore) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(Self::new_inner(table_store))),
        }
    }

    pub async fn init(&self) -> Result<(), String> {
        trace!("Crypto::init");

        // make local copy of node id for easy access
        let (table_store, node_id) = {
            let mut inner = self.inner.lock();
            let c = self.config.get();
            inner.node_id = c.network.node_id;
            inner.node_id_secret = c.network.node_id_secret;
            (inner.table_store.clone(), c.network.node_id)
        };

        // load caches if they are valid for this node id
        let mut db = table_store.open("crypto_caches", 1).await?;
        let caches_valid = match db.load(0, b"node_id").await? {
            Some(v) => v.as_slice() == node_id.bytes,
            None => false,
        };
        if caches_valid {
            if let Some(b) = db.load(0, b"dh_cache").await? {
                let mut inner = self.inner.lock();
                bytes_to_cache(&b, &mut inner.dh_cache);
            }
        } else {
            drop(db);
            table_store.delete("crypto_caches").await?;
            db = table_store.open("crypto_caches", 1).await?;
            db.store(0, b"node_id", &node_id.bytes).await?;
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

    pub async fn flush(&self) -> Result<(), String> {
        //trace!("Crypto::flush");
        let (table_store, cache_bytes) = {
            let inner = self.inner.lock();
            let cache_bytes = cache_to_bytes(&inner.dh_cache);
            (inner.table_store.clone(), cache_bytes)
        };

        let db = table_store.open("crypto_caches", 1).await?;
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

    fn ed25519_to_x25519_pk(key: &ed::PublicKey) -> Result<xd::PublicKey, String> {
        let bytes = key.to_bytes();
        let compressed = cd::edwards::CompressedEdwardsY(bytes);
        let point = compressed
            .decompress()
            .ok_or_else(fn_string!("ed25519_to_x25519_pk failed"))?;
        let mp = point.to_montgomery();
        Ok(xd::PublicKey::from(mp.to_bytes()))
    }
    fn ed25519_to_x25519_sk(key: &ed::SecretKey) -> Result<xd::StaticSecret, String> {
        let exp = ed::ExpandedSecretKey::from(key);
        let bytes: [u8; ed::EXPANDED_SECRET_KEY_LENGTH] = exp.to_bytes();
        let lowbytes: [u8; 32] = bytes[0..32].try_into().map_err(map_to_string)?;
        Ok(xd::StaticSecret::from(lowbytes))
    }

    pub fn cached_dh(&self, key: &DHTKey, secret: &DHTKeySecret) -> Result<SharedSecret, String> {
        if let Some(c) = self
            .inner
            .lock()
            .dh_cache
            .find(|entry| entry.key == *key && entry.secret == *secret)
        {
            return Ok(c.shared_secret);
        }

        let shared_secret = Self::compute_dh(key, secret)?;
        self.inner.lock().dh_cache.insert(DHCacheEntry {
            key: *key,
            secret: *secret,
            shared_secret,
        });
        Ok(shared_secret)
    }

    ///////////
    // These are safe to use regardless of initialization status

    pub fn compute_dh(key: &DHTKey, secret: &DHTKeySecret) -> Result<SharedSecret, String> {
        assert!(key.valid);
        assert!(secret.valid);
        let pk_ed = ed::PublicKey::from_bytes(&key.bytes).map_err(map_to_string)?;
        let pk_xd = Self::ed25519_to_x25519_pk(&pk_ed)?;
        let sk_ed = ed::SecretKey::from_bytes(&secret.bytes).map_err(map_to_string)?;
        let sk_xd = Self::ed25519_to_x25519_sk(&sk_ed)?;
        Ok(sk_xd.diffie_hellman(&pk_xd).to_bytes())
    }

    pub fn get_random_nonce() -> Nonce {
        let mut nonce = [0u8; 24];
        random_bytes(&mut nonce).unwrap();
        nonce
    }

    pub fn get_random_secret() -> SharedSecret {
        let mut s = [0u8; 32];
        random_bytes(&mut s).unwrap();
        s
    }

    pub fn decrypt_in_place(
        body: &mut Vec<u8>,
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<(), String> {
        let key = ch::Key::from(*shared_secret);
        let xnonce = ch::XNonce::from(*nonce);
        let aead = ch::XChaCha20Poly1305::new(&key);
        aead.decrypt_in_place(&xnonce, associated_data.unwrap_or(b""), body)
            .map_err(map_to_string)
            .map_err(logthru_crypto!())
    }

    pub fn decrypt(
        body: &[u8],
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let mut out = body.to_vec();
        Self::decrypt_in_place(&mut out, nonce, shared_secret, associated_data)
            .map_err(map_to_string)
            .map_err(logthru_crypto!())?;
        Ok(out)
    }

    pub fn encrypt_in_place(
        body: &mut Vec<u8>,
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<(), String> {
        let key = ch::Key::from(*shared_secret);
        let xnonce = ch::XNonce::from(*nonce);
        let aead = ch::XChaCha20Poly1305::new(&key);

        aead.encrypt_in_place(&xnonce, associated_data.unwrap_or(b""), body)
            .map_err(map_to_string)
            .map_err(logthru_crypto!())
    }

    pub fn encrypt(
        body: &[u8],
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let mut out = body.to_vec();
        Self::encrypt_in_place(&mut out, nonce, shared_secret, associated_data)
            .map_err(map_to_string)
            .map_err(logthru_crypto!())?;
        Ok(out)
    }
}

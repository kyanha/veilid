use super::test_veilid_config::*;
use crate::*;

async fn startup() -> VeilidAPI {
    trace!("test_table_store: starting");
    let (update_callback, config_callback) = setup_veilid_core();
    api_startup(update_callback, config_callback)
        .await
        .expect("startup failed")
}

async fn shutdown(api: VeilidAPI) {
    trace!("test_table_store: shutting down");
    api.shutdown().await;
    trace!("test_table_store: finished");
}

pub async fn test_delete_open_delete(ts: TableStore) {
    trace!("test_delete_open_delete");

    let _ = ts.delete("test");
    let db = ts.open("test", 3).await.expect("should have opened");
    assert!(
        ts.delete("test").await.is_err(),
        "should fail because file is opened"
    );
    drop(db);
    assert!(
        ts.delete("test").await.is_ok(),
        "should succeed because file is closed"
    );
    let db = ts.open("test", 3).await.expect("should have opened");
    assert!(
        ts.delete("test").await.is_err(),
        "should fail because file is opened"
    );
    drop(db);
    let db = ts.open("test", 3).await.expect("should have opened");
    assert!(
        ts.delete("test").await.is_err(),
        "should fail because file is opened"
    );
    drop(db);
    assert!(
        ts.delete("test").await.is_ok(),
        "should succeed because file is closed"
    );
}

pub async fn test_store_delete_load(ts: TableStore) {
    trace!("test_store_delete_load");

    let _ = ts.delete("test");
    let db = ts.open("test", 3).await.expect("should have opened");
    assert!(
        ts.delete("test").await.is_err(),
        "should fail because file is opened"
    );

    assert_eq!(
        db.load(0, b"foo").unwrap(),
        None,
        "should not load missing key"
    );
    assert!(
        db.store(1, b"foo", b"1234567890").await.is_ok(),
        "should store new key"
    );
    assert_eq!(
        db.load(0, b"foo").unwrap(),
        None,
        "should not load missing key"
    );
    assert_eq!(db.load(1, b"foo").unwrap(), Some(b"1234567890".to_vec()));

    assert!(
        db.store(1, b"bar", b"FNORD").await.is_ok(),
        "should store new key"
    );
    assert!(
        db.store(0, b"bar", b"ABCDEFGHIJKLMNOPQRSTUVWXYZ")
            .await
            .is_ok(),
        "should store new key"
    );
    assert!(
        db.store(2, b"bar", b"FNORD").await.is_ok(),
        "should store new key"
    );
    assert!(
        db.store(2, b"baz", b"QWERTY").await.is_ok(),
        "should store new key"
    );
    assert!(
        db.store(2, b"bar", b"QWERTYUIOP").await.is_ok(),
        "should store new key"
    );

    assert_eq!(db.load(1, b"bar").unwrap(), Some(b"FNORD".to_vec()));
    assert_eq!(
        db.load(0, b"bar").unwrap(),
        Some(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec())
    );
    assert_eq!(db.load(2, b"bar").unwrap(), Some(b"QWERTYUIOP".to_vec()));
    assert_eq!(db.load(2, b"baz").unwrap(), Some(b"QWERTY".to_vec()));

    assert_eq!(db.delete(1, b"bar").await.unwrap(), true);
    assert_eq!(db.delete(1, b"bar").await.unwrap(), false);
    assert!(
        db.delete(4, b"bar").await.is_err(),
        "can't delete from column that doesn't exist"
    );

    drop(db);
    let db = ts.open("test", 3).await.expect("should have opened");

    assert_eq!(db.load(1, b"bar").unwrap(), None);
    assert_eq!(
        db.load(0, b"bar").unwrap(),
        Some(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec())
    );
    assert_eq!(db.load(2, b"bar").unwrap(), Some(b"QWERTYUIOP".to_vec()));
    assert_eq!(db.load(2, b"baz").unwrap(), Some(b"QWERTY".to_vec()));
}

pub async fn test_frozen(vcrypto: CryptoSystemVersion, ts: TableStore) {
    trace!("test_frozen");

    let _ = ts.delete("test");
    let db = ts.open("test", 3).await.expect("should have opened");
    let keypair = vcrypto.generate_keypair();

    assert!(db.store_rkyv(0, b"asdf", &keypair).await.is_ok());

    assert_eq!(db.load_rkyv::<KeyPair>(0, b"qwer").unwrap(), None);

    let d = match db.load_rkyv::<KeyPair>(0, b"asdf") {
        Ok(x) => x,
        Err(e) => {
            panic!("couldn't decode: {}", e);
        }
    };
    assert_eq!(d, Some(keypair), "keys should be equal");

    assert!(
        db.store(1, b"foo", b"1234567890").await.is_ok(),
        "should store new key"
    );

    assert!(
        db.load_rkyv::<TypedKey>(1, b"foo").is_err(),
        "should fail to unfreeze"
    );
}

pub async fn test_all() {
    let api = startup().await;
    let crypto = api.crypto().unwrap();
    let ts = api.table_store().unwrap();

    for ck in VALID_CRYPTO_KINDS {
        let vcrypto = crypto.get(ck).unwrap();
        test_delete_open_delete(ts.clone()).await;
        test_store_delete_load(ts.clone()).await;
        test_frozen(vcrypto, ts.clone()).await;
        let _ = ts.delete("test").await;
    }

    shutdown(api).await;
}

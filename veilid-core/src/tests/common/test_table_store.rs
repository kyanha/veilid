use super::test_veilid_config::*;
use crate::dht::key;
use crate::intf::*;
use crate::xx::*;
use crate::*;

fn setup_veilid_core() -> VeilidCoreSetup {
    VeilidCoreSetup {
        update_callback: Arc::new(
            move |veilid_update: VeilidUpdate| -> SystemPinBoxFuture<()> {
                Box::pin(async move {
                    trace!("update_callback: {:?}", veilid_update);
                })
            },
        ),
        config_callback: Arc::new(config_callback),
    }
}

async fn startup(core: VeilidCore) -> VeilidAPI {
    trace!("test_table_store: starting");
    core.startup(setup_veilid_core())
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
        db.load(0, b"foo").await,
        Ok(None),
        "should not load missing key"
    );
    assert!(
        db.store(1, b"foo", b"1234567890").await.is_ok(),
        "should store new key"
    );
    assert_eq!(
        db.load(0, b"foo").await,
        Ok(None),
        "should not load missing key"
    );
    assert_eq!(db.load(1, b"foo").await, Ok(Some(b"1234567890".to_vec())));

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

    assert_eq!(db.load(1, b"bar").await, Ok(Some(b"FNORD".to_vec())));
    assert_eq!(
        db.load(0, b"bar").await,
        Ok(Some(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec()))
    );
    assert_eq!(db.load(2, b"bar").await, Ok(Some(b"QWERTYUIOP".to_vec())));
    assert_eq!(db.load(2, b"baz").await, Ok(Some(b"QWERTY".to_vec())));

    assert_eq!(db.delete(1, b"bar").await, Ok(true));
    assert_eq!(db.delete(1, b"bar").await, Ok(false));
    assert!(
        db.delete(4, b"bar").await.is_err(),
        "can't delete from column that doesn't exist"
    );

    drop(db);
    let db = ts.open("test", 3).await.expect("should have opened");

    assert_eq!(db.load(1, b"bar").await, Ok(None));
    assert_eq!(
        db.load(0, b"bar").await,
        Ok(Some(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec()))
    );
    assert_eq!(db.load(2, b"bar").await, Ok(Some(b"QWERTYUIOP".to_vec())));
    assert_eq!(db.load(2, b"baz").await, Ok(Some(b"QWERTY".to_vec())));
}

pub async fn test_cbor(ts: TableStore) {
    trace!("test_cbor");

    let _ = ts.delete("test");
    let db = ts.open("test", 3).await.expect("should have opened");
    let (dht_key, _) = key::generate_secret();

    assert!(db.store_cbor(0, b"asdf", &dht_key).await.is_ok());

    assert_eq!(db.load_cbor::<key::DHTKey>(0, b"qwer").await, Ok(None));

    let d = match db.load_cbor::<key::DHTKey>(0, b"asdf").await {
        Ok(x) => x,
        Err(e) => {
            panic!("couldn't decode cbor: {}", e);
        }
    };
    assert_eq!(d, Some(dht_key), "keys should be equal");

    assert!(
        db.store(1, b"foo", b"1234567890").await.is_ok(),
        "should store new key"
    );

    assert!(
        db.load_cbor::<key::DHTKey>(1, b"foo").await.is_err(),
        "should fail to load cbor"
    );
}

pub async fn test_all() {
    let core = VeilidCore::new();
    let api = startup(core.clone()).await;

    let ts = core.table_store();
    test_delete_open_delete(ts.clone()).await;
    test_store_delete_load(ts.clone()).await;
    test_cbor(ts.clone()).await;

    let _ = ts.delete("test").await;

    shutdown(api).await;
}

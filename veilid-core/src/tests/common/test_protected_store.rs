use super::test_veilid_config::*;
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

async fn startup() -> VeilidAPI {
    trace!("test_table_store: starting");
    api_startup(setup_veilid_core())
        .await
        .expect("startup failed")
}

async fn shutdown(api: VeilidAPI) {
    trace!("test_table_store: shutting down");
    api.shutdown().await;
    trace!("test_table_store: finished");
}

pub async fn test_protected_store(ps: ProtectedStore) {
    info!("testing protected store");

    let _ = ps.remove_user_secret("_test_key").await;
    let _ = ps.remove_user_secret("_test_broken").await;

    let d1: [u8; 0] = [];

    assert_eq!(
        ps.save_user_secret("_test_key", &[2u8, 3u8, 4u8]).await,
        Ok(false)
    );
    info!("testing saving user secret");
    assert_eq!(ps.save_user_secret("_test_key", &d1).await, Ok(true));
    info!("testing loading user secret");
    assert_eq!(
        ps.load_user_secret("_test_key").await,
        Ok(Some(d1.to_vec()))
    );
    info!("testing loading user secret again");
    assert_eq!(
        ps.load_user_secret("_test_key").await,
        Ok(Some(d1.to_vec()))
    );
    info!("testing loading broken user secret");
    assert_eq!(ps.load_user_secret("_test_broken").await, Ok(None));
    info!("testing loading broken user secret again");
    assert_eq!(ps.load_user_secret("_test_broken").await, Ok(None));
    info!("testing remove user secret");
    assert_eq!(ps.remove_user_secret("_test_key").await, Ok(true));
    info!("testing remove user secret again");
    assert_eq!(ps.remove_user_secret("_test_key").await, Ok(false));
    info!("testing remove broken user secret");
    assert_eq!(ps.remove_user_secret("_test_broken").await, Ok(false));
    info!("testing remove broken user secret again");
    assert_eq!(ps.remove_user_secret("_test_broken").await, Ok(false));

    let d2: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    assert_eq!(
        ps.save_user_secret("_test_key", &[2u8, 3u8, 4u8]).await,
        Ok(false)
    );
    assert_eq!(ps.save_user_secret("_test_key", &d2).await, Ok(true));
    assert_eq!(
        ps.load_user_secret("_test_key").await,
        Ok(Some(d2.to_vec()))
    );
    assert_eq!(
        ps.load_user_secret("_test_key").await,
        Ok(Some(d2.to_vec()))
    );
    assert_eq!(ps.load_user_secret("_test_broken").await, Ok(None));
    assert_eq!(ps.load_user_secret("_test_broken").await, Ok(None));
    assert_eq!(ps.remove_user_secret("_test_key").await, Ok(true));
    assert_eq!(ps.remove_user_secret("_test_key").await, Ok(false));
    assert_eq!(ps.remove_user_secret("_test_broken").await, Ok(false));
    assert_eq!(ps.remove_user_secret("_test_broken").await, Ok(false));

    let _ = ps.remove_user_secret("_test_key").await;
    let _ = ps.remove_user_secret("_test_broken").await;
}

pub async fn test_all() {
    let api = startup().await;
    let ps = api.protected_store().unwrap();
    test_protected_store(ps.clone()).await;

    shutdown(api).await;
}

use super::test_veilid_config::*;
use crate::xx::*;
use crate::*;

pub async fn test_startup_shutdown() {
    trace!("test_startup_shutdown: starting");
    let veilid_core = VeilidCore::new();
    let api = veilid_core
        .startup(setup_veilid_core())
        .await
        .expect("startup failed");
    trace!("test_startup_shutdown: shutting down");
    api.shutdown().await;
    trace!("test_startup_shutdown: finished");
}

pub async fn test_attach_detach() {
    let veilid_core = VeilidCore::new();

    info!("--- test normal order ---");
    let api = veilid_core
        .startup(setup_veilid_core())
        .await
        .expect("startup failed");
    api.attach().await.unwrap();
    intf::sleep(5000).await;
    api.detach().await.unwrap();
    api.wait_for_state(VeilidState::Attachment(AttachmentState::Detached))
        .await
        .unwrap();
    api.shutdown().await;

    info!("--- test auto detach ---");
    let api = veilid_core
        .startup(setup_veilid_core())
        .await
        .expect("startup failed");
    api.attach().await.unwrap();
    intf::sleep(5000).await;
    api.shutdown().await;

    info!("--- test detach without attach ---");
    let api = veilid_core
        .startup(setup_veilid_core())
        .await
        .expect("startup failed");
    api.detach().await.unwrap();
    api.shutdown().await;
}

pub async fn test_all() {
    test_startup_shutdown().await;
    test_attach_detach().await;
}

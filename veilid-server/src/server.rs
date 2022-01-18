use crate::client_api;
use crate::settings::*;
use crate::veilid_logs::*;
use async_std::channel::{bounded, Receiver, Sender};
use lazy_static::*;
use log::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use veilid_core::xx::SingleShotEventual;

lazy_static! {
    static ref SHUTDOWN_SWITCH: Mutex<Option<SingleShotEventual<()>>> =
        Mutex::new(Some(SingleShotEventual::new(())));
}

pub fn shutdown() {
    let shutdown_switch = SHUTDOWN_SWITCH.lock().take();
    if let Some(shutdown_switch) = shutdown_switch {
        shutdown_switch.resolve(());
    }
}

pub async fn run_veilid_server(settings: Settings, logs: VeilidLogs) -> Result<(), String> {
    let settingsr = settings.read();

    // Create Veilid Core
    let veilid_core = veilid_core::VeilidCore::new();

    // Create client api state change pipe
    let (sender, receiver): (
        Sender<veilid_core::VeilidUpdate>,
        Receiver<veilid_core::VeilidUpdate>,
    ) = bounded(1);

    // Create VeilidCore setup
    let vcs = veilid_core::VeilidCoreSetup {
        update_callback: Arc::new(
            move |change: veilid_core::VeilidUpdate| -> veilid_core::SystemPinBoxFuture<()> {
                let sender = sender.clone();
                Box::pin(async move {
                    if sender.send(change).await.is_err() {
                        error!("error sending veilid update callback");
                    }
                })
            },
        ),
        config_callback: settings.get_core_config_callback(),
    };

    // Start Veilid Core and get API
    let veilid_api = veilid_core
        .startup(vcs)
        .await
        .map_err(|e| format!("VeilidCore startup failed: {}", e))?;

    // Start client api if one is requested
    let mut capi = if settingsr.client_api.enabled {
        let some_capi = client_api::ClientApi::new(veilid_api.clone());
        some_capi
            .clone()
            .run(settingsr.client_api.listen_address.addrs.clone());
        Some(some_capi)
    } else {
        None
    };

    // Drop rwlock on settings
    let auto_attach = settingsr.auto_attach;
    drop(settingsr);

    // Handle state changes on main thread for capnproto rpc
    let update_receiver_jh = capi.clone().map(|capi| {
        async_std::task::spawn_local(async move {
            while let Ok(change) = receiver.recv().await {
                capi.clone().handle_update(change);
            }
        })
    });
    // Handle log messages on main thread for capnproto rpc
    let client_log_receiver_jh = capi
        .clone()
        .map(|capi| {
            logs.client_log_channel
                .clone()
                .map(|mut client_log_channel| {
                    async_std::task::spawn_local(async move {
                        // Batch messages to either 16384 chars at once or every second to minimize packets
                        let rate = Duration::from_secs(1);
                        let mut start = Instant::now();
                        let mut messages = String::new();
                        loop {
                            let timeout_dur =
                                rate.checked_sub(start.elapsed()).unwrap_or(Duration::ZERO);
                            match async_std::future::timeout(timeout_dur, client_log_channel.recv())
                                .await
                            {
                                Ok(Ok(message)) => {
                                    messages += &message;
                                    if messages.len() > 16384 {
                                        capi.clone()
                                            .handle_client_log(core::mem::take(&mut messages));
                                        start = Instant::now();
                                    }
                                }
                                Ok(Err(_)) => break,
                                Err(_) => {
                                    capi.clone()
                                        .handle_client_log(core::mem::take(&mut messages));
                                    start = Instant::now();
                                }
                            }
                        }
                    })
                })
        })
        .flatten();

    // Auto-attach if desired
    if auto_attach {
        info!("Auto-attach to the Veilid network");
        if let Err(e) = veilid_api.attach().await {
            error!("Auto-attaching to the Veilid network failed: {:?}", e);
            shutdown();
        }
    }

    // Idle while waiting to exit
    let shutdown_switch = {
        let shutdown_switch_locked = SHUTDOWN_SWITCH.lock();
        (*shutdown_switch_locked).as_ref().map(|ss| ss.instance())
    };
    if let Some(shutdown_switch) = shutdown_switch {
        shutdown_switch.await;
    }

    // Stop the client api if we have one
    if let Some(c) = capi.as_mut().cloned() {
        c.stop().await;
    }

    // Shut down Veilid API to release state change sender
    veilid_api.shutdown().await;

    // Close the client api log channel if it is open to release client log sender
    if let Some(client_log_channel_closer) = logs.client_log_channel_closer {
        client_log_channel_closer.close();
    }

    // Wait for update receiver to exit
    if let Some(update_receiver_jh) = update_receiver_jh {
        update_receiver_jh.await;
    }

    // Wait for client api log receiver to exit
    if let Some(client_log_receiver_jh) = client_log_receiver_jh {
        client_log_receiver_jh.await;
    }

    Ok(())
}

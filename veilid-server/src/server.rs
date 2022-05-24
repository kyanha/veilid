use crate::client_api;
use crate::settings::*;
use crate::veilid_logs::*;
use flume::{bounded, Receiver, Sender};
use lazy_static::*;
use log::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use veilid_core::xx::SingleShotEventual;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ServerMode {
    Normal,
    ShutdownImmediate,
    DumpTXTRecord,
}

lazy_static! {
    static ref SHUTDOWN_SWITCH: Mutex<Option<SingleShotEventual<()>>> =
        Mutex::new(Some(SingleShotEventual::new(Some(()))));
}

pub fn shutdown() {
    let shutdown_switch = SHUTDOWN_SWITCH.lock().take();
    if let Some(shutdown_switch) = shutdown_switch {
        shutdown_switch.resolve(());
    }
}

pub async fn run_veilid_server(
    settings: Settings,
    logs: VeilidLogs,
    server_mode: ServerMode,
) -> Result<(), String> {
    let settingsr = settings.read();

    // Create client api state change pipe
    let (sender, receiver): (
        Sender<veilid_core::VeilidUpdate>,
        Receiver<veilid_core::VeilidUpdate>,
    ) = bounded(1);

    // Create VeilidCore setup
    let update_callback = Arc::new(move |change: veilid_core::VeilidUpdate| {
        if sender.send(change).is_err() {
            error!("error sending veilid update callback");
        }
    });
    let config_callback = settings.get_core_config_callback();

    // Start Veilid Core and get API
    let veilid_api = veilid_core::api_startup(update_callback, config_callback)
        .await
        .map_err(|e| format!("VeilidCore startup failed: {}", e))?;

    // Start client api if one is requested
    let mut capi = if settingsr.client_api.enabled && matches!(server_mode, ServerMode::Normal) {
        let some_capi = client_api::ClientApi::new(veilid_api.clone());
        some_capi
            .clone()
            .run(settingsr.client_api.listen_address.addrs.clone());
        Some(some_capi)
    } else {
        None
    };

    // Drop rwlock on settings
    let auto_attach = settingsr.auto_attach || !matches!(server_mode, ServerMode::Normal);
    drop(settingsr);

    // Process all updates
    let capi2 = capi.clone();
    let update_receiver_jh = async_std::task::spawn_local(async move {
        while let Ok(change) = receiver.recv_async().await {
            if let Some(capi) = &capi2 {
                // Handle state changes on main thread for capnproto rpc
                capi.clone().handle_update(change);
            }
        }
    });
    // Handle log messages on main thread for capnproto rpc
    let client_log_receiver_jh = capi.clone().and_then(|capi| {
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
    });

    // Auto-attach if desired
    let mut out = Ok(());
    if auto_attach {
        info!("Auto-attach to the Veilid network");
        if let Err(e) = veilid_api.attach().await {
            let outerr = format!("Auto-attaching to the Veilid network failed: {:?}", e);
            error!("{}", outerr);
            out = Err(outerr);
            shutdown();
        }
    }

    // Process dump-txt-record
    if matches!(server_mode, ServerMode::DumpTXTRecord) {
        let start_time = Instant::now();
        while Instant::now().duration_since(start_time) < Duration::from_secs(10) {
            match veilid_api.get_state().await {
                Ok(vs) => {
                    if vs.network.started {
                        break;
                    }
                }
                Err(e) => {
                    let outerr = format!("Getting state failed: {:?}", e);
                    error!("{}", outerr);
                    out = Err(outerr);
                    break;
                }
            }
            async_std::task::sleep(Duration::from_millis(100)).await;
        }
        match veilid_api.debug("txtrecord".to_string()).await {
            Ok(v) => {
                print!("{}", v);
            }
            Err(e) => {
                let outerr = format!("Getting TXT record failed: {:?}", e);
                error!("{}", outerr);
                out = Err(outerr);
            }
        };
        shutdown();
    }

    // Process shutdown-immediate
    if matches!(server_mode, ServerMode::ShutdownImmediate) {
        shutdown();
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
    update_receiver_jh.await;

    // Wait for client api log receiver to exit
    if let Some(client_log_receiver_jh) = client_log_receiver_jh {
        client_log_receiver_jh.await;
    }

    out
}

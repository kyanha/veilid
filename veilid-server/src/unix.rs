#[cfg(unix)]
use crate::client_api;
use crate::client_log_channel::*;
use crate::settings;
use async_std::channel::{bounded, Receiver, Sender};
use clap::{App, Arg};
use lazy_static::*;
use log::*;
use parking_lot::Mutex;
use simplelog::*;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use veilid_core::xx::SingleShotEventual;

fn parse_command_line(default_config_path: &OsStr) -> Result<clap::ArgMatches, clap::Error> {
    let matches = App::new("veilid-server")
        .version("0.1")
        .about("Veilid Server")
        .arg(
            Arg::with_name("daemon")
                .long("daemon")
                .short("d")
                .help("Run in daemon mode in the background"),
        )
        .arg(
            Arg::with_name("subnode_index")
                .long("subnode_index")
                .takes_value(true)
                .help("Run as an extra daemon on the same machine for testing purposes, specify a number greater than zero to offset the listening ports"),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .help("Turn on debug logging on the terminal"),
        )
        .arg(
            Arg::with_name("trace")
                .long("trace")
                .conflicts_with("debug")
                .help("Turn on trace logging on the terminal"),
        )
        .arg(
            Arg::with_name("generate-id")
                .long("generate-id")
                .help("Only generate a new node id and print it"),
        )
        .arg(
            Arg::with_name("config-file")
                .short("c")
                .long("config-file")
                .takes_value(true)
                .value_name("FILE")
                .default_value_os(default_config_path)
                .help("Specify a configuration file to use"),
        )
        .arg(
            Arg::with_name("bootstrap")
                .long("bootstrap")
                .takes_value(true)
                .value_name("BOOTSTRAP_LIST")
                .help("Specify a list of bootstrap servers to use"),
        )
        .arg(
            Arg::with_name("attach")
                .long("attach")
                .takes_value(true)
                .value_name("BOOL")
                .possible_values(&["false", "true"])
                .help("Automatically attach the server to the Veilid network"),
        )
        ;
    #[cfg(debug_assertions)]
    let matches = matches.arg(
        Arg::with_name("wait-for-debug")
            .long("wait-for-debug")
            .help("Wait for debugger to attach"),
    );
    Ok(matches.get_matches())
}

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

pub async fn main() -> Result<(), String> {
    // Wait until signal
    ctrlc::set_handler(move || {
        shutdown();
    })
    .expect("Error setting Ctrl-C handler");

    // Get command line options
    let default_config_path = settings::Settings::get_default_config_path();
    let matches = parse_command_line(default_config_path.as_os_str())
        .map_err(|e| format!("failed to parse command line: {}", e))?;

    // Check for one-off commands
    #[cfg(debug_assertions)]
    if matches.occurrences_of("wait-for-debug") != 0 {
        use bugsalot::debugger;
        debugger::wait_until_attached(None).expect("state() not implemented on this platform");
    }
    if matches.occurrences_of("generate-id") != 0 {
        let (key, secret) = veilid_core::generate_secret();
        println!("Public: {}\nSecret: {}", key.encode(), secret.encode());
        return Ok(());
    }

    // Attempt to load configuration
    let settings = settings::Settings::new(
        matches.occurrences_of("config-file") == 0,
        matches.value_of_os("config-file").unwrap(),
    )
    .map_err(|e| format!("configuration is invalid: {}", e))?;

    // write lock the settings
    let mut settingsrw = settings.write();

    // Set config from command line
    if matches.occurrences_of("daemon") != 0 {
        settingsrw.daemon = true;
        settingsrw.logging.terminal.enabled = false;
    }
    if matches.occurrences_of("subnode_index") != 0 {
        let subnode_index = match matches.value_of("subnode_index") {
            Some(x) => x
                .parse()
                .map_err(|e| format!("couldn't parse subnode index: {}", e))?,
            None => {
                return Err("value not specified for subnode_index".to_owned());
            }
        };
        if subnode_index == 0 {
            return Err("value of subnode_index should be between 1 and 65535".to_owned());
        }
        settingsrw.testing.subnode_index = subnode_index;
    }
    if matches.occurrences_of("debug") != 0 {
        settingsrw.logging.terminal.enabled = true;
        settingsrw.logging.terminal.level = settings::LogLevel::Debug;
    }
    if matches.occurrences_of("trace") != 0 {
        settingsrw.logging.terminal.enabled = true;
        settingsrw.logging.terminal.level = settings::LogLevel::Trace;
    }
    if matches.is_present("attach") {
        settingsrw.auto_attach = !matches!(matches.value_of("attach"), Some("false"));
    }
    if matches.occurrences_of("bootstrap") != 0 {
        let bootstrap = match matches.value_of("bootstrap") {
            Some(x) => {
                println!("Overriding bootstrap with: ");
                let mut out: Vec<settings::ParsedUrl> = Vec::new();
                for x in x.split(',') {
                    println!("    {}", x);
                    out.push(settings::ParsedUrl::from_str(x).map_err(|e| {
                        format!("unable to parse url in bootstrap list: {} for {}", e, x)
                    })?);
                }
                out
            }
            None => {
                return Err("value not specified for bootstrap".to_owned());
            }
        };
        settingsrw.core.network.bootstrap = bootstrap;
    }

    // Apply subnode index if we're testing
    drop(settingsrw);
    settings
        .apply_subnode_index()
        .map_err(|_| "failed to apply subnode index".to_owned())?;
    let settingsr = settings.read();

    // Set up loggers
    let mut logs: Vec<Box<dyn SharedLogger>> = Vec::new();
    let mut client_log_channel: Option<ClientLogChannel> = None;
    let mut client_log_channel_closer: Option<ClientLogChannelCloser> = None;
    let mut cb = ConfigBuilder::new();
    cb.add_filter_ignore_str("async_std");
    cb.add_filter_ignore_str("async_io");
    cb.add_filter_ignore_str("polling");
    cb.add_filter_ignore_str("rustls");
    cb.add_filter_ignore_str("async_tungstenite");
    cb.add_filter_ignore_str("tungstenite");

    if settingsr.logging.terminal.enabled {
        logs.push(TermLogger::new(
            settings::convert_loglevel(settingsr.logging.terminal.level),
            cb.build(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ))
    }
    if settingsr.logging.file.enabled {
        let log_path = Path::new(&settingsr.logging.file.path);

        let logfile;
        if settingsr.logging.file.append {
            logfile = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .map_err(|e| format!("failed to open log file: {}", e))?
        } else {
            logfile = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(log_path)
                .map_err(|e| format!("failed to open log file: {}", e))?
        }
        logs.push(WriteLogger::new(
            settings::convert_loglevel(settingsr.logging.file.level),
            cb.build(),
            logfile,
        ))
    }
    if settingsr.logging.client.enabled {
        let (clog, clogwriter, clogcloser) = ClientLogChannel::new();
        client_log_channel = Some(clog);
        client_log_channel_closer = Some(clogcloser);
        logs.push(WriteLogger::new(
            settings::convert_loglevel(settingsr.logging.client.level),
            cb.build(),
            clogwriter,
        ))
    }
    CombinedLogger::init(logs).map_err(|e| format!("failed to init logs: {}", e))?;

    // Create Veilid Core
    let veilid_core = veilid_core::VeilidCore::new();

    // Create client api state change pipe
    let (sender, receiver): (
        Sender<veilid_core::VeilidStateChange>,
        Receiver<veilid_core::VeilidStateChange>,
    ) = bounded(1);

    // Create VeilidCore setup
    let vcs = veilid_core::VeilidCoreSetup {
        state_change_callback: Arc::new(
            move |change: veilid_core::VeilidStateChange| -> veilid_core::SystemPinBoxFuture<()> {
                let sender = sender.clone();
                Box::pin(async move {
                    if sender.send(change).await.is_err() {
                        error!("error sending state change callback");
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
    let state_change_receiver_jh = capi.clone().map(|capi| {
        async_std::task::spawn_local(async move {
            while let Ok(change) = receiver.recv().await {
                capi.clone().handle_state_change(change);
            }
        })
    });
    // Handle log messages on main thread for capnproto rpc
    let client_log_receiver_jh = capi
        .clone()
        .map(|capi| {
            client_log_channel.take().map(|mut client_log_channel| {
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
    if let Some(client_log_channel_closer) = client_log_channel_closer {
        client_log_channel_closer.close();
    }

    // Wait for state change receiver to exit
    if let Some(state_change_receiver_jh) = state_change_receiver_jh {
        state_change_receiver_jh.await;
    }

    // Wait for client api log receiver to exit
    if let Some(client_log_receiver_jh) = client_log_receiver_jh {
        client_log_receiver_jh.await;
    }

    Ok(())
}

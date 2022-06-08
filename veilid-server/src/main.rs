#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![deny(unused_must_use)]

mod client_api;
mod cmdline;
mod log_safe_channel;
mod server;
mod settings;
#[cfg(unix)]
mod unix;
mod veilid_logs;
#[cfg(windows)]
mod windows;

use async_std::task;
use cfg_if::*;
use server::*;
use veilid_logs::*;

#[allow(clippy::all)]
pub mod veilid_client_capnp {
    include!(concat!(env!("OUT_DIR"), "/proto/veilid_client_capnp.rs"));
}

fn main() -> Result<(), String> {
    #[cfg(windows)]
    let _ = ansi_term::enable_ansi_support();

    let (settings, matches) = cmdline::process_command_line()?;

    // --- Dump Config ---
    if matches.occurrences_of("dump-config") != 0 {
        return serde_yaml::to_writer(std::io::stdout(), &*settings.read())
            .map_err(|e| e.to_string());
    }

    // --- Generate DHT Key ---
    if matches.occurrences_of("generate-dht-key") != 0 {
        let (key, secret) = veilid_core::generate_secret();
        println!("Public: {}\nSecret: {}", key.encode(), secret.encode());
        return Ok(());
    }

    // See if we're just running a quick command
    let (server_mode, success, failure) = if matches.occurrences_of("set-node-id") != 0 {
        (
            ServerMode::ShutdownImmediate,
            "Node Id and Secret set successfully",
            "Failed to set Node Id and Secret",
        )
    } else if matches.occurrences_of("dump-txt-record") != 0 {
        (ServerMode::DumpTXTRecord, "", "Failed to dump txt record")
    } else {
        (ServerMode::Normal, "", "")
    };

    // Handle non-normal server modes
    if !matches!(server_mode, ServerMode::Normal) {
        // Init combined console/file logger
        let logs = VeilidLogs::setup(settings.clone())?;
        // run the server to set the node id and quit
        return task::block_on(async { run_veilid_server(settings, logs, server_mode).await })
            .map(|v| {
                println!("{}", success);
                v
            })
            .map_err(|e| {
                println!("{}", failure);
                e
            });
    }

    // --- Daemon Mode ----
    if settings.read().daemon.enabled {
        cfg_if! {
            if #[cfg(windows)] {
                return windows::run_service(settings, matches).map_err(|e| format!("{}", e));
            } else if #[cfg(unix)] {
                return unix::run_daemon(settings, matches);
            }
        }
    }

    // Init combined console/file logger
    let logs = VeilidLogs::setup(settings.clone())?;

    // --- Normal Startup ---
    ctrlc::set_handler(move || {
        shutdown();
    })
    .expect("Error setting Ctrl-C handler");

    // Run the server loop
    task::block_on(async { run_veilid_server(settings, logs, server_mode).await })
}

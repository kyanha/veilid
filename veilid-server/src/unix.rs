use crate::server::*;
use crate::settings::Settings;
use crate::veilid_logs::*;
use async_std::stream::StreamExt;
use async_std::task;
use clap::ArgMatches;
use signal_hook::consts::signal::*;
use signal_hook_async_std::Signals;
use std::io::Read;
use tracing::*;

#[instrument(skip(signals))]
async fn handle_signals(mut signals: Signals) {
    while let Some(signal) = signals.next().await {
        match signal {
            SIGHUP => {
                // XXX: reload configuration?
            }
            SIGTERM | SIGINT | SIGQUIT => {
                // Shutdown the system;
                shutdown();
            }
            _ => unreachable!(),
        }
    }
}

#[instrument(err)]
pub fn run_daemon(settings: Settings, _matches: ArgMatches) -> Result<(), String> {
    let daemon = {
        let mut daemon = daemonize::Daemonize::new();
        let s = settings.read();
        if let Some(pid_file) = s.daemon.pid_file.clone() {
            daemon = daemon.pid_file(pid_file.clone()); //.chown_pid_file(true);
            daemon = daemon.exit_action(move || {
                // wait for pid file to exist before exiting parent
                let pid_path = std::path::Path::new(&pid_file);
                loop {
                    if let Ok(mut f) = std::fs::File::open(pid_path) {
                        let mut s = String::new();
                        if f.read_to_string(&mut s).is_ok()
                            && !s.is_empty()
                            && s.parse::<u32>().is_ok()
                        {
                            println!("pidfile found");
                            break;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            })
        }
        if let Some(chroot) = &s.daemon.chroot {
            daemon = daemon.chroot(chroot);
        }
        if let Some(working_directory) = &s.daemon.working_directory {
            daemon = daemon.working_directory(working_directory);
        }
        if let Some(user) = &s.daemon.user {
            daemon = daemon.user(user.as_str());
        }
        if let Some(group) = &s.daemon.group {
            daemon = daemon.group(group.as_str());
        }

        let stdout_file = if let Some(stdout_file) = &s.daemon.stdout_file {
            Some(
                std::fs::File::create(stdout_file)
                    .map_err(|e| format!("Failed to create stdio file: {}", e))?,
            )
        } else {
            None
        };
        if let Some(stderr_file) = &s.daemon.stderr_file {
            if Some(stderr_file) == s.daemon.stdout_file.as_ref() {
                // same output file for stderr and stdout
                daemon = daemon.stderr(
                    stdout_file
                        .as_ref()
                        .unwrap()
                        .try_clone()
                        .map_err(|e| format!("Failed to clone stdout file: {}", e))?,
                );
            } else {
                daemon = daemon.stderr(
                    std::fs::File::create(stderr_file)
                        .map_err(|e| format!("Failed to create stderr file: {}", e))?,
                );
            }
        }
        if let Some(stdout_file) = stdout_file {
            daemon = daemon.stdout(stdout_file);
        }

        daemon
    };

    // Init combined console/file logger
    let _logs = VeilidLogs::setup(settings.clone())?;

    // Daemonize
    daemon
        .start()
        .map_err(|e| format!("Failed to daemonize: {}", e))?;

    // Now, run the server
    task::block_on(async {
        // Catch signals
        let signals = Signals::new(&[SIGHUP, SIGTERM, SIGINT, SIGQUIT])
            .map_err(|e| format!("failed to init signals: {}", e))?;
        let handle = signals.handle();

        let signals_task = async_std::task::spawn(handle_signals(signals));

        let res = run_veilid_server(settings, ServerMode::Normal).await;

        // Terminate the signal stream.
        handle.close();
        signals_task.await;

        res
    })
}

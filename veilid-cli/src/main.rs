#![deny(clippy::all)]
#![deny(unused_must_use)]

use anyhow::*;
use async_std::prelude::*;
use clap::{App, Arg};
use flexi_logger::*;
use log::*;
use std::ffi::OsStr;
use std::net::ToSocketAddrs;

mod client_api_connection;
mod command_processor;
mod settings;
mod ui;

#[allow(clippy::all)]
pub mod veilid_client_capnp {
    include!(concat!(env!("OUT_DIR"), "/proto/veilid_client_capnp.rs"));
}

fn parse_command_line(default_config_path: &OsStr) -> Result<clap::ArgMatches, clap::Error> {
    let matches = App::new("veilid-cli")
        .version("0.1")
        .about("Veilid Console Client")
        .arg(
            Arg::with_name("address")
                .required(false)
                .help("Address to connect to"),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .help("Turn on debug logging"),
        )
        .arg(
            Arg::with_name("wait-for-debug")
                .long("wait-for-debug")
                .help("Wait for debugger to attach"),
        )
        .arg(
            Arg::with_name("trace")
                .long("trace")
                .conflicts_with("debug")
                .help("Turn on trace logging"),
        )
        .arg(
            Arg::with_name("config-file")
                .short("c")
                .takes_value(true)
                .value_name("FILE")
                .default_value_os(default_config_path)
                .help("Specify a configuration file to use"),
        )
        .get_matches();

    Ok(matches)
}

#[async_std::main]
async fn main() -> Result<()> {
    // Get command line options
    let default_config_path = settings::Settings::get_default_config_path();
    let matches = parse_command_line(default_config_path.as_os_str())?;
    if matches.occurrences_of("wait-for-debug") != 0 {
        use bugsalot::debugger;
        debugger::wait_until_attached(None).expect("state() not implemented on this platform");
    }

    // Attempt to load configuration
    let mut settings = settings::Settings::new(
        matches.occurrences_of("config-file") == 0,
        matches.value_of_os("config-file").unwrap(),
    )
    .map_err(Box::new)?;

    // Set config from command line
    if matches.occurrences_of("debug") != 0 {
        settings.logging.level = settings::LogLevel::Debug;
        settings.logging.terminal.enabled = true;
    }
    if matches.occurrences_of("trace") != 0 {
        settings.logging.level = settings::LogLevel::Trace;
        settings.logging.terminal.enabled = true;
    }

    // Create UI object
    let mut sivui = ui::UI::new(settings.interface.node_log.scrollback, &settings);

    // Set up loggers
    {
        let mut specbuilder = LogSpecBuilder::new();
        specbuilder.default(settings::convert_loglevel(settings.logging.level));
        specbuilder.module("cursive_core", LevelFilter::Off);
        specbuilder.module("cursive_buffered_backend", LevelFilter::Off);
        specbuilder.module("mio", LevelFilter::Off);
        specbuilder.module("async_std", LevelFilter::Off);
        specbuilder.module("async_io", LevelFilter::Off);
        specbuilder.module("polling", LevelFilter::Off);

        let logger = Logger::with(specbuilder.build());

        if settings.logging.terminal.enabled {
            let flv = sivui.cursive_flexi_logger();
            if settings.logging.file.enabled {
                std::fs::create_dir_all(settings.logging.file.directory.clone())?;
                logger
                    .log_target(LogTarget::FileAndWriter(flv))
                    .suppress_timestamp()
                    //    .format(flexi_logger::colored_default_format)
                    .directory(settings.logging.file.directory.clone())
                    .start()
                    .expect("failed to initialize logger!");
            } else {
                logger
                    .log_target(LogTarget::Writer(flv))
                    .suppress_timestamp()
                    .format(flexi_logger::colored_default_format)
                    .start()
                    .expect("failed to initialize logger!");
            }
        } else if settings.logging.file.enabled {
            std::fs::create_dir_all(settings.logging.file.directory.clone())?;
            logger
                .log_target(LogTarget::File)
                .suppress_timestamp()
                .directory(settings.logging.file.directory.clone())
                .start()
                .expect("failed to initialize logger!");
        }
    }
    // Get client address
    let server_addrs;
    if let Some(address_arg) = matches.value_of("address") {
        server_addrs = address_arg
            .to_socket_addrs()
            .context(format!("Invalid server address '{}'", address_arg))?
            .collect()
    } else {
        server_addrs = settings.address.addrs.clone();
    }
    let server_addr = server_addrs.first().cloned();

    // Create command processor
    debug!("Creating Command Processor ");
    let mut comproc = command_processor::CommandProcessor::new(sivui.clone(), &settings);
    sivui.set_command_processor(comproc.clone());

    // Create client api client side
    info!("Starting API connection");
    let mut capi = client_api_connection::ClientApiConnection::new(comproc.clone());

    // Save client api in command processor
    comproc.set_client_api_connection(capi.clone());

    // Keep a connection to the server
    comproc.set_server_address(server_addr);
    let mut comproc2 = comproc.clone();
    let connection_future = comproc.connection_manager();
    // Start UI
    let ui_future = async_std::task::spawn_local(async move {
        sivui.run_async().await;

        // When UI quits, close connection and command processor cleanly
        comproc2.quit();
        capi.disconnect().await;
    });

    // Wait for ui and connection to complete
    ui_future.join(connection_future).await;

    Ok(())
}

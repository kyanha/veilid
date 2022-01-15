use crate::settings::*;
use std::ffi::OsStr;
use clap::{App, Arg, ArgMatches};
use std::str::FromStr;

fn do_clap_matches(default_config_path: &OsStr) -> Result<clap::ArgMatches, clap::Error> {

    let matches = App::new("veilid-server")
        .version("0.1")
        .about("Veilid Server")
        .color(clap::ColorChoice::Auto)
        .arg(
            Arg::new("daemon")
                .long("daemon")
                .short('d')
                .help("Run in daemon mode in the background"),
        )
        .arg(
            Arg::new("config-file")
                .short('c')
                .long("config-file")
                .takes_value(true)
                .value_name("FILE")
                .default_value_os(default_config_path)
                .help("Specify a configuration file to use"),
        ).arg(
            Arg::new("attach")
                .long("attach")
                .takes_value(true)
                .value_name("BOOL")
                .possible_values(&["false", "true"])
                .help("Automatically attach the server to the Veilid network"),
        )
        // Dev options
        .arg(
            Arg::new("debug")
                .long("debug")
                .help("Turn on debug logging on the terminal"),
        )
        .arg(
            Arg::new("trace")
                .long("trace")
                .conflicts_with("debug")
                .help("Turn on trace logging on the terminal"),
        )
        .arg(
            Arg::new("subnode_index")
                .long("subnode_index")
                .takes_value(true)
                .help("Run as an extra daemon on the same machine for testing purposes, specify a number greater than zero to offset the listening ports"),
        )
        .arg(
            Arg::new("generate-dht-key")
                .long("generate-dht-key")
                .help("Only generate a new dht key and print it"),
        )
        
        .arg(
            Arg::new("dump-config")
                .long("dump-config")
                .help("Instead of running the server, print the configuration it would use to the console"),
        )
        .arg(
            Arg::new("bootstrap")
                .long("bootstrap")
                .takes_value(true)
                .value_name("BOOTSTRAP_LIST")
                .help("Specify a list of bootstrap servers to use"),
        )
        .arg(
            Arg::new("local")
                .long("local")
                .help("Enable local peer scope")
        );
        
        #[cfg(debug_assertions)]
        let matches = matches.arg(
            Arg::new("wait-for-debug")
                .long("wait-for-debug")
                .help("Wait for debugger to attach"),
        );
       
    Ok(matches.get_matches())
}

pub fn process_command_line() -> Result<(Settings, ArgMatches), String> {
   
    // Get command line options
    let default_config_path = Settings::get_default_config_path();
    let matches = do_clap_matches(default_config_path.as_os_str())
        .map_err(|e| format!("failed to parse command line: {}", e))?;

    // Check for one-off commands
    #[cfg(debug_assertions)]
    if matches.occurrences_of("wait-for-debug") != 0 {
        use bugsalot::debugger;
        debugger::wait_until_attached(None).expect("state() not implemented on this platform");
    }

    // Attempt to load configuration
    let settings = Settings::new(
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
        settingsrw.logging.terminal.level = LogLevel::Debug;
    }
    if matches.occurrences_of("trace") != 0 {
        settingsrw.logging.terminal.enabled = true;
        settingsrw.logging.terminal.level = LogLevel::Trace;
    }
    if matches.is_present("attach") {
        settingsrw.auto_attach = !matches!(matches.value_of("attach"), Some("true"));
    }
    if matches.is_present("local") {
        settingsrw.core.network.enable_local_peer_scope = true;
    }
    if matches.occurrences_of("bootstrap") != 0 {
        let bootstrap = match matches.value_of("bootstrap") {
            Some(x) => {
                println!("Overriding bootstrap with: ");
                let mut out: Vec<ParsedNodeDialInfo> = Vec::new();
                for x in x.split(',') {
                    println!("    {}", x);
                    out.push(ParsedNodeDialInfo::from_str(x).map_err(|e| {
                        format!(
                            "unable to parse dial info in bootstrap list: {} for {}",
                            e, x
                        )
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

    Ok((settings, matches))
}
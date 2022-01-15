use crate::settings::Settings;
use clap::ArgMatches;
// use log::*;

pub fn run_daemon(_settings: Settings, _matches: ArgMatches) -> Result<(), String> {
    eprintln!("Windows Service mode not implemented yet.");
    Ok(())
}

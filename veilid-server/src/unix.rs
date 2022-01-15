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

pub fn run_daemon(settings: Settings, matches: ArgMatches) -> Result<(), String> {
    eprintln!("Windows Service mode not implemented yet.");
    Ok(())
}

//! rc-watchdog: Windows SYSTEM service that monitors rc-agent process health.
//!
//! Polls for rc-agent.exe every 5 seconds. On crash detection, spawns
//! start-rcagent.bat in Session 1 via WTSQueryUserToken + CreateProcessAsUser,
//! then sends a WatchdogCrashReport to rc-core via HTTP POST.

mod reporter;
mod service;
mod session;

use std::ffi::OsString;
use tracing_appender::rolling;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use windows_service::{define_windows_service, service_dispatcher};

define_windows_service!(ffi_service_main, service_main);

fn main() -> Result<(), windows_service::Error> {
    // When running as a service, SCM calls ffi_service_main which delegates to service_main.
    // service_dispatcher::start blocks until the service is stopped.
    service_dispatcher::start("RCWatchdog", ffi_service_main)?;
    Ok(())
}

fn service_main(arguments: Vec<OsString>) {
    // Initialize file-based tracing (rolling daily log at C:\RacingPoint\watchdog.log)
    let file_appender = rolling::daily(r"C:\RacingPoint", "watchdog.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(false);

    tracing_subscriber::registry().with(subscriber).init();

    tracing::info!("RCWatchdog service_main called");

    if let Err(e) = service::run(arguments) {
        tracing::error!("Service error: {}", e);
    }
}

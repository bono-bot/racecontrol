//! rc-watchdog: Windows service (pod mode) + James monitor (standalone mode).
//!
//! With --service flag: Windows SYSTEM service monitoring rc-agent on pods.
//! Without --service flag: James monitor mode (Task Scheduler, single-shot).

mod bono_alert;
mod failure_state;
mod james_monitor;
mod reporter;
mod service;
mod session;

use std::ffi::OsString;
use tracing_subscriber::prelude::*;
use windows_service::{define_windows_service, service_dispatcher};

define_windows_service!(ffi_service_main, service_main);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(|s| s.as_str()) == Some("--service") {
        // Pod watchdog mode: run as Windows service
        // Initialize file-based tracing (rolling daily log)
        let file_appender =
            tracing_appender::rolling::daily(r"C:\RacingPoint", "watchdog.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        let subscriber = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(false);
        tracing_subscriber::registry()
            .with(subscriber)
            .init();

        service_dispatcher::start("RCWatchdog", ffi_service_main)?;
    } else {
        // James monitor mode: single-shot check run (Task Scheduler every 2min)
        let file_appender = tracing_appender::rolling::daily(
            r"C:\Users\bono\.claude",
            "rc-watchdog.log",
        );
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        let subscriber = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(false);
        tracing_subscriber::registry()
            .with(subscriber)
            .init();

        james_monitor::run_monitor();
    }

    Ok(())
}

fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = service::run(arguments) {
        tracing::error!("Service error: {}", e);
    }
}

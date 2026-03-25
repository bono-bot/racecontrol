//! rc-watchdog: Windows service (pod mode) + James monitor (standalone mode).
//!
//! With --service flag: Windows SYSTEM service monitoring rc-agent on pods.
//! Without --service flag: James monitor mode (persistent daemon, checks every 2min).

mod bono_alert;
mod failure_state;
mod james_monitor;
mod reporter;
mod service;
mod session;

use std::ffi::OsString;
use tracing_subscriber::prelude::*;
use windows_service::{define_windows_service, service_dispatcher};

#[cfg(windows)]
mod singleton {
    use std::ptr;
    use winapi::um::synchapi::CreateMutexW;
    use winapi::um::synchapi::ReleaseMutex;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::shared::minwindef::TRUE;
    use winapi::shared::winerror::ERROR_ALREADY_EXISTS;

    /// Windows mutex-based singleton guard. Drop closes the handle.
    pub struct SingletonGuard(winapi::shared::ntdef::HANDLE);

    impl SingletonGuard {
        /// Try to acquire a named mutex. Returns None if another instance holds it.
        pub fn try_acquire(name: &str) -> Option<Self> {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                let handle = CreateMutexW(ptr::null_mut(), TRUE, wide.as_ptr());
                if handle.is_null() {
                    return None;
                }
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    CloseHandle(handle);
                    return None;
                }
                Some(Self(handle))
            }
        }
    }

    impl Drop for SingletonGuard {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }
}

const BUILD_ID: &str = env!("GIT_HASH");

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

        tracing::info!("RCWatchdog service starting (build {})", BUILD_ID);
        service_dispatcher::start("RCWatchdog", ffi_service_main)?;
    } else {
        // James monitor mode: persistent daemon with internal 2-min loop

        // Singleton guard — prevent multiple instances
        #[cfg(windows)]
        let _singleton = match singleton::SingletonGuard::try_acquire("Global\\RCWatchdogJames") {
            Some(guard) => guard,
            None => {
                eprintln!("rc-watchdog: another instance is already running, exiting");
                std::process::exit(0);
            }
        };

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

        tracing::info!("rc-watchdog james daemon starting (build {})", BUILD_ID);
        loop {
            james_monitor::run_monitor();
            std::thread::sleep(std::time::Duration::from_secs(120));
        }
    }

    Ok(())
}

fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = service::run(arguments) {
        tracing::error!("Service error: {}", e);
    }
}

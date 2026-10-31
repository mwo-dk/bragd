//! `bragd-helper`: the Helper. A window starts it when it finds none; the session starts it
//! with `--stay` when *Start with my session* is on. `--service on|off` registers it with the
//! session or takes the registration away; `--service` alone prints `on` or `off`, and says it
//! by its exit code too (0 on, 3 off). What `--service` did goes into `helper.log` as well.
//!
//! A release build on Windows is a GUI program, so starting it never opens a console window;
//! it has no console to print to there, which is why the exit code and the log say it too.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

use bragd_helper::{helper, service};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let done = match args.as_slice() {
        [] => helper::serve(false).map(drop),
        [s] if *s == service::STAY => helper::serve(true).map(drop),
        ["--service"] => {
            let on = service::installed();
            let said = if on { "on" } else { "off" };
            println!("{said}");
            bragd_helper::log_line(&format!("--service: {said}"));
            if !on {
                return ExitCode::from(OFF);
            }
            Ok(())
        }
        ["--service", "on"] => logged("--service on", std::env::current_exe().and_then(|exe| service::install(&exe))),
        ["--service", "off"] => logged("--service off", service::uninstall()),
        _ => {
            eprintln!("usage: bragd-helper [--stay | --service [on|off]]");
            return ExitCode::from(2);
        }
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bragd-helper: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--service` alone: the Helper is not registered.
const OFF: u8 = 3;

/// `done`, written into `helper.log` too: on Windows nobody sees what a GUI program prints.
fn logged(what: &str, done: std::io::Result<()>) -> std::io::Result<()> {
    bragd_helper::log_line(&match &done {
        Ok(()) => format!("{what}: done"),
        Err(e) => format!("{what}: {e}"),
    });
    done
}

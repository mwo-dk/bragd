//! `bragd-helper`: the Helper. A window starts it when it finds none; the session starts it
//! with `--stay` when *Start with my session* is on. `--service on|off` registers it with the
//! session or takes the registration away; `--service` alone prints `on` or `off`.

use std::process::ExitCode;

use bragd_helper::{helper, service};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let done = match args.as_slice() {
        [] => helper::serve(false).map(drop),
        [s] if *s == service::STAY => helper::serve(true).map(drop),
        ["--service"] => {
            println!("{}", if service::installed() { "on" } else { "off" });
            Ok(())
        }
        ["--service", "on"] => std::env::current_exe().and_then(|exe| service::install(&exe)),
        ["--service", "off"] => service::uninstall(),
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

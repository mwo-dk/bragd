//! A fake Lichess, Lishogi or OGS on 127.0.0.1, for screenshots and trying the Play online page
//! by hand:
//!
//!     cargo run -p bragd-online --features fake --example fake-online -- lichess 8765
//!     BRAGD_LICHESS_URL=http://127.0.0.1:8765 BRAGD_LICHESS_TOKEN=lip_fake bragd-gui
//!
//! `lishogi` takes `BRAGD_LISHOGI_URL` and `BRAGD_LISHOGI_TOKEN` (`lip_fake`), `ogs` takes
//! `BRAGD_OGS_URL` and `BRAGD_OGS_TOKEN` (`ogs_fake`). It prints its address, then serves until
//! it is stopped.

fn main() {
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_default();
    let port = args.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let fake = match server.as_str() {
        "lichess" => bragd_online::fake::lichess_on(port),
        "lishogi" => bragd_online::fake::lishogi_on(port),
        "ogs" => bragd_online::fake::ogs_on(port),
        _ => {
            eprintln!("usage: fake-online lichess|lishogi|ogs [port]");
            std::process::exit(2);
        }
    };
    println!("{}", fake.url);
    loop {
        std::thread::park();
    }
}

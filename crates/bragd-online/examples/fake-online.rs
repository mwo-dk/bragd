//! A fake Lichess, Lishogi, OGS or Bragd server on 127.0.0.1, for screenshots and trying the Play online page
//! by hand:
//!
//!     cargo run -p bragd-online --features fake --example fake-online -- lichess 8765
//!     BRAGD_LICHESS_URL=http://127.0.0.1:8765 BRAGD_LICHESS_TOKEN=lip_fake bragd-gui
//!
//! `lishogi` takes `BRAGD_LISHOGI_URL` and `BRAGD_LISHOGI_TOKEN` (`lip_fake`), `ogs` takes
//! `BRAGD_OGS_URL` and `BRAGD_OGS_TOKEN` (`ogs_fake`), `bragd` takes `BRAGD_FRIENDS_URL` and
//! `BRAGD_FRIENDS_TOKEN` (`fake-refresh`), or the password `bragd-fake`; `bragd-nameless` is the
//! same account before it chose a name, and `bragd-dice` has a backgammon Game against Anna
//! under way, Black to throw. It prints its address, then serves until
//! it is stopped.

fn main() {
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_default();
    let port = args.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let fake = match server.as_str() {
        "lichess" => bragd_online::fake::lichess_on(port),
        "lishogi" => bragd_online::fake::lishogi_on(port),
        "ogs" => bragd_online::fake::ogs_on(port),
        "bragd" | "bragd-nameless" | "bragd-dice" => {
            let (fake, world) = bragd_online::fake_bragd::bragd_on(port);
            let mut w = world.lock().unwrap_or_else(|e| e.into_inner());
            w.agree = true;
            if server == "bragd-nameless" {
                w.me = None;
            }
            // Backgammon against Anna, Black to throw with the cube in the middle.
            if server == "bragd-dice" {
                let g = &mut w.games[0];
                (g.variant, g.status, g.first, g.initial) = ("backgammon".into(), "playing".into(), false, Some("white".into()));
                g.moves = vec!["3-1".into(), "8/5 6/5".into()];
            }
            drop(w);
            fake
        }
        _ => {
            eprintln!("usage: fake-online lichess|lishogi|ogs|bragd|bragd-nameless|bragd-dice [port]");
            std::process::exit(2);
        }
    };
    println!("{}", fake.url);
    loop {
        std::thread::park();
    }
}

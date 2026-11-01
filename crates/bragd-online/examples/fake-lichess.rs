//! The fake Lichess on 127.0.0.1, for screenshots and trying the Play online page by hand:
//!
//!     cargo run -p bragd-online --features fake --example fake-lichess -- 8765
//!     BRAGD_LICHESS_URL=http://127.0.0.1:8765 BRAGD_LICHESS_TOKEN=lip_fake bragd-gui
//!
//! It prints its address, then serves until it is stopped.

fn main() {
    let port = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let fake = bragd_online::fake::lichess_on(port);
    println!("{}", fake.url);
    loop {
        std::thread::park();
    }
}

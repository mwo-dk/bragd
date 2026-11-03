//! `bragd-server`: the game service, configured by its environment (packaging/server/README.md).

use bragd_server::api::{Public, Service, Turn};
use bragd_server::{db::Pool, http, jwt::Verifier};

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn main() {
    let supabase_url = var("BRAGD_SERVER_SUPABASE_URL").trim_end_matches('/').to_string();
    let database = var("BRAGD_SERVER_DATABASE_URL");
    if supabase_url.is_empty() || database.is_empty() {
        eprintln!("bragd-server: set BRAGD_SERVER_SUPABASE_URL and BRAGD_SERVER_DATABASE_URL");
        std::process::exit(2);
    }
    let jwks = format!("{supabase_url}/auth/v1/.well-known/jwks.json");
    let secret = var("BRAGD_SERVER_JWT_SECRET");
    let public = Public {
        supabase_url,
        supabase_key: var("BRAGD_SERVER_SUPABASE_KEY"),
        oauth: var("BRAGD_SERVER_OAUTH").split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
    };
    let pool = match Pool::new(&database) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("bragd-server: {e}");
            std::process::exit(2);
        }
    };
    let turn = Turn {
        urls: var("BRAGD_SERVER_TURN_URLS").split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
        secret: var("BRAGD_SERVER_TURN_SECRET"),
    };
    let service = Service { pool, verifier: Verifier::new(Some(&secret), Some(&jwks)), public, turn };
    let listen = std::env::var("BRAGD_SERVER_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    eprintln!("bragd-server: listening on {listen}");
    if let Err(e) = http::serve(service, &listen, 8) {
        eprintln!("bragd-server: {e}");
        std::process::exit(1);
    }
}

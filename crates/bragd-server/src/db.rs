//! The connection to the Bragd server's Postgres, and a request run as its caller.
//!
//! A caller's request runs in a transaction as the role `authenticated` with their JWT claims in
//! `request.jwt.claims`, exactly as Supabase's own API runs it, so the row-level security in
//! `supabase/migrations/` decides what they may read and write. The referee's writes step out of
//! that role ([`referee`]) only after the request has read, as the caller, what it acts on.

use postgres::{Client, Transaction};
use std::sync::{Arc, Mutex};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::jwt::Caller;

/// Idle connections, kept for the next request.
pub struct Pool {
    url: String,
    tls: MakeRustlsConnect,
    idle: Mutex<Vec<Client>>,
}

/// How many idle connections are kept: the free plan allows about sixty in all.
const IDLE: usize = 8;

impl Pool {
    /// `url` is a libpq connection string; `sslmode=require` for Supabase.
    pub fn new(url: &str) -> Result<Pool, String> {
        use rustls_platform_verifier::BuilderVerifierExt;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .and_then(|b| b.with_platform_verifier())
            .map_err(|e| format!("TLS: {e}"))?
            .with_no_client_auth();
        Ok(Pool { url: url.to_string(), tls: MakeRustlsConnect::new(config), idle: Mutex::new(vec![]) })
    }

    fn get(&self) -> Result<Client, postgres::Error> {
        let kept = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
        match kept {
            Some(c) if !c.is_closed() => Ok(c),
            _ => Client::connect(&self.url, self.tls.clone()),
        }
    }

    fn put(&self, c: Client) {
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() < IDLE && !c.is_closed() {
            idle.push(c);
        }
    }

    /// Runs `f` in one transaction as `caller`, and commits when it returns `Ok`.
    pub fn as_user<T, E: From<postgres::Error>>(&self, caller: &Caller, f: impl FnOnce(&mut Transaction) -> Result<T, E>) -> Result<T, E> {
        let mut c = self.get()?;
        let out = {
            let mut tx = c.transaction()?;
            tx.batch_execute("set local role authenticated")?;
            tx.execute("select set_config('request.jwt.claims', $1, true)", &[&caller.claims])?;
            let out = f(&mut tx)?;
            tx.commit()?;
            out
        };
        self.put(c);
        Ok(out)
    }
}

/// Steps out of the caller's role, for the referee's own writes: what the caller may not do
/// themselves, such as recording a move the rules found legal.
pub fn referee(tx: &mut Transaction) -> Result<(), postgres::Error> {
    tx.batch_execute("reset role")
}

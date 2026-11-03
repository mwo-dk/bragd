//! The Bragd server's game service. It referees Games between Bragd players with Bragd's own
//! rules, on the same Postgres as the Bragd server's Supabase project, and checks each caller by
//! the access token Supabase Auth gave them (`docs/adr/0005-bragd-server-on-hosted-supabase.md`).

pub mod api;
pub mod db;
pub mod friends;
pub mod http;
pub mod jwt;
pub mod turn;

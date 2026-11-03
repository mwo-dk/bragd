//! A throwaway plain Postgres for the tests: the one named by `BRAGD_TEST_PG` (a CI service),
//! or a container of `IMAGE` started with podman or docker and removed by its own id at the end.
//! Each case gets a database of its own, copied from a template that holds Supabase's stand-ins
//! (`supabase-stub.sql`) and every migration in `supabase/migrations/`.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bragd_server::api::{Public, Service, Turn};
use bragd_server::db::Pool;
use bragd_server::jwt::{Verifier, sign_hs256, user_claims};
use postgres::{Client, NoTls};
use serde_json::{Value, json};

pub const IMAGE: &str = "docker.io/library/postgres:17-alpine";
pub const SECRET: &str = "test-jwt-secret";

/// The server, and the container to remove when done.
pub struct Pg {
    /// The address without a database name: `postgres://postgres:…@127.0.0.1:port`.
    base: String,
    container: Option<(String, String)>,
    next: AtomicUsize,
}

impl Drop for Pg {
    fn drop(&mut self) {
        if let Some((tool, id)) = &self.container {
            let _ = Command::new(tool).args(["rm", "-f", id]).output();
        }
    }
}

fn run(tool: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(tool).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl Pg {
    pub fn start() -> Pg {
        if let Ok(url) = std::env::var("BRAGD_TEST_PG") {
            let pg = Pg { base: url.trim_end_matches('/').to_string(), container: None, next: AtomicUsize::new(0) };
            pg.template();
            return pg;
        }
        let tool = ["podman", "docker"].into_iter().find(|t| run(t, &["version"]).is_some()).expect(
            "the database tests need podman or docker to run a throwaway Postgres, or BRAGD_TEST_PG set to a server",
        );
        let id = run(tool, &["run", "-d", "--rm", "-e", "POSTGRES_PASSWORD=bragd", "-p", "127.0.0.1::5432", IMAGE])
            .unwrap_or_else(|| panic!("{tool} could not start {IMAGE}"));
        let mut pg = Pg { base: String::new(), container: Some((tool.to_string(), id.clone())), next: AtomicUsize::new(0) };
        let port = run(tool, &["port", &id, "5432/tcp"]).and_then(|p| p.lines().next().and_then(|l| l.rsplit(':').next()).map(str::to_string)).expect("the container's port");
        pg.base = format!("postgres://postgres:bragd@127.0.0.1:{port}");
        // A guard against a hang only: the server is ready when it answers.
        let until = Instant::now() + Duration::from_secs(90);
        loop {
            match Client::connect(&format!("{}/postgres", pg.base), NoTls).map(|mut c| c.simple_query("select 1").is_ok()) {
                Ok(true) => break,
                _ if Instant::now() > until => panic!("Postgres in {id} did not start"),
                _ => std::thread::sleep(Duration::from_millis(300)),
            }
        }
        pg.template();
        pg
    }

    fn admin(&self, db: &str) -> Client {
        Client::connect(&format!("{}/{db}", self.base), NoTls).expect("a connection")
    }

    /// The template database: Supabase's stand-ins and every migration, in order.
    fn template(&self) {
        let mut c = self.admin("postgres");
        c.batch_execute("drop database if exists bragd_template").unwrap();
        c.batch_execute("create database bragd_template").unwrap();
        let mut t = self.admin("bragd_template");
        // Roles live in the cluster, not the database: made once.
        let have: bool = c.query_one("select exists (select 1 from pg_roles where rolname = 'authenticated')", &[]).unwrap().get(0);
        let stub = include_str!("supabase-stub.sql");
        let stub = if have { stub.lines().filter(|l| !l.starts_with("create role")).collect::<Vec<_>>().join("\n") } else { stub.to_string() };
        t.batch_execute(&stub).expect("the Supabase stand-ins");
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../supabase/migrations");
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "sql")).collect();
        files.sort();
        for f in files {
            let sql = std::fs::read_to_string(&f).unwrap();
            t.batch_execute(&sql).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
        }
    }

    /// A fresh database for one case.
    pub fn fresh(&self) -> Db {
        let name = format!("case_{}_{}", std::process::id(), self.next.fetch_add(1, Ordering::SeqCst));
        self.admin("postgres").batch_execute(&format!("create database {name} template bragd_template")).unwrap();
        let url = format!("{}/{name}", self.base);
        let service = Service {
            pool: Pool::new(&format!("{url}?sslmode=disable")).unwrap(),
            verifier: Verifier::new(Some(SECRET), None),
            public: Public { supabase_url: "http://supabase.test".into(), supabase_key: "publishable".into(), oauth: vec!["github".into()] },
            turn: Turn { urls: vec!["stun:turn.test:3478".into(), "turn:turn.test:3478".into(), "turns:turn.test:5349".into()], secret: "turn-secret".into() },
        };
        Db { url, service }
    }
}

/// One case's database, and the game service on it.
pub struct Db {
    pub url: String,
    pub service: Service,
}

/// A signed-in player.
pub struct User {
    pub id: uuid::Uuid,
    pub token: String,
}

impl Db {
    /// A connection as the database's owner, outside row-level security.
    pub fn owner(&self) -> Client {
        Client::connect(&self.url, NoTls).unwrap()
    }

    /// A user Supabase Auth knows, with a token, and no profile yet.
    pub fn account(&self) -> User {
        let id = uuid::Uuid::new_v4();
        self.owner().execute("insert into auth.users (id) values ($1)", &[&id]).unwrap();
        let token = sign_hs256(SECRET, &user_claims(id, 4_000_000_000));
        User { id, token }
    }

    /// A user with the profile `name`.
    pub fn user(&self, name: &str) -> User {
        let u = self.account();
        let (status, v) = self.call(&u, "PUT", "/v1/me", json!({ "name": name }));
        assert_eq!(status, 200, "{v}");
        u
    }

    pub fn call(&self, u: &User, method: &str, path: &str, body: Value) -> (u16, Value) {
        self.service.handle(method, path, Some(&format!("Bearer {}", u.token)), &body.to_string())
    }

    /// Runs `sql` as `u` would through Supabase's own API: role `authenticated`, their claims set.
    pub fn sql_as(&self, u: &User, sql: &str) -> Result<Vec<postgres::Row>, postgres::Error> {
        self.sql_on(u, "", sql)
    }

    /// [`Db::sql_as`] on the Realtime topic `topic`, as Realtime runs a join's check.
    pub fn sql_on(&self, u: &User, topic: &str, sql: &str) -> Result<Vec<postgres::Row>, postgres::Error> {
        let mut c = self.owner();
        let mut tx = c.transaction()?;
        tx.batch_execute("set local role authenticated")?;
        let claims = user_claims(u.id, 4_000_000_000).to_string();
        tx.execute("select set_config('request.jwt.claims', $1, true)", &[&claims])?;
        tx.execute("select set_config('realtime.topic', $1, true)", &[&topic])?;
        let rows = tx.query(sql, &[])?;
        tx.commit()?;
        Ok(rows)
    }
}

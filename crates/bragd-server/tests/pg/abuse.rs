//! What one player may not do to the others through Supabase's own API or the game service:
//! forge a row, write without end, store junk, or hold the service up.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::json;

use crate::support::{Db, Pg};

pub const CASES: &[crate::Case] = &[
    ("no_table_is_open_beyond_its_grants", no_table_is_open_beyond_its_grants),
    ("the_one_asked_cannot_forge_another_friendship", the_one_asked_cannot_forge_another_friendship),
    ("a_player_writes_only_the_columns_granted", a_player_writes_only_the_columns_granted),
    ("a_variant_is_a_short_key", a_variant_is_a_short_key),
    ("an_unplayable_variant_is_refused_cleanly", an_unplayable_variant_is_refused_cleanly),
    ("writes_are_capped_for_each_player", writes_are_capped_for_each_player),
    ("challenges_cannot_push_games_under_way_out", challenges_cannot_push_games_under_way_out),
    ("old_challenges_and_invites_expire", old_challenges_and_invites_expire),
    ("a_stalled_request_holds_up_nobody", a_stalled_request_holds_up_nobody),
];

/// No API role may truncate, reference or trigger on any table, and `anon` has nothing at all,
/// whatever Supabase's default privileges granted.
fn no_table_is_open_beyond_its_grants(pg: &Pg) {
    let db = pg.fresh();
    let open: Vec<String> = db
        .owner()
        .query(
            "select format('%s %s %s', r, t.tablename, p) from pg_tables t, unnest(array['anon', 'authenticated']) r, \
             unnest(array['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'TRUNCATE', 'REFERENCES', 'TRIGGER']) p \
             where t.schemaname = 'public' and has_table_privilege(r, format('public.%I', t.tablename), p) \
             and (r = 'anon' or p in ('TRUNCATE', 'REFERENCES', 'TRIGGER'))",
            &[],
        )
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(open, Vec::<String>::new());
}

fn the_one_asked_cannot_forge_another_friendship(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let carl = db.user("Carl");
    db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" }));
    let (a, b) = (anna.id.min(bo.id), anna.id.max(bo.id));
    let (x, y) = (bo.id.min(carl.id), bo.id.max(carl.id));
    let forge = format!("update public.friendships set a = '{x}', b = '{y}', asked_by = '{}', accepted = true where a = '{a}' and b = '{b}' returning 1", carl.id);
    let _ = db.sql_as(&bo, &forge);
    let friends: bool = db.owner().query_one("select bragd.are_friends($1, $2)", &[&bo.id, &carl.id]).unwrap().get(0);
    assert!(!friends, "Bo made himself Carl's Friend");
    // Accepting what he was asked still works.
    assert_eq!(db.call(&bo, "POST", "/v1/friends", json!({ "name": "Anna" })).1, json!({ "friends": true }));
}

fn a_player_writes_only_the_columns_granted(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let forever = format!("insert into public.seeks (player, variant, created_at) values ('{}', 'chess', now() + interval '10 years')", anna.id);
    assert!(db.sql_as(&anna, &forever).is_err(), "a seek that never expires");
    let invite = format!("insert into public.invites (code, inviter, created_at) values ('abcdefgh12', '{}', now() + interval '10 years')", anna.id);
    assert!(db.sql_as(&anna, &invite).is_err(), "an invite that never expires");
    let said = "update public.profiles set created_at = now() - interval '9 years'";
    assert!(db.sql_as(&anna, said).is_err());
    // What the game service writes as the caller still goes in.
    assert_eq!(db.call(&anna, "POST", "/v1/seeks", json!({ "variant": "chess" })).0, 200);
    assert_eq!(db.call(&anna, "POST", "/v1/invites", json!({})).0, 200);
    assert_eq!(db.call(&anna, "POST", "/v1/blocks", json!({ "name": "Bo" })).0, 200);
    assert_eq!(db.call(&anna, "POST", "/v1/reports", json!({ "name": "Bo", "reason": "rude" })).0, 200);
    assert_eq!(db.call(&anna, "POST", "/v1/blocks/remove", json!({ "name": "Bo" })).0, 200);
    assert_eq!(db.call(&bo, "POST", "/v1/clubs", json!({ "name": "Skakklubben" })).0, 200);
}

fn a_variant_is_a_short_key(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let long = "x".repeat(10_000);
    let challenge = |v: &str| format!("insert into public.games (first, second, challenger, variant) values ('{0}', '{1}', '{0}', '{v}')", anna.id, bo.id);
    let seek = |v: &str| format!("insert into public.seeks (player, variant) values ('{}', '{v}')", anna.id);
    for v in [long.as_str(), "Chess Variant", "", "chess\n"] {
        assert!(db.sql_as(&anna, &challenge(v)).is_err(), "challenge {:?}", &v[..v.len().min(20)]);
        assert!(db.sql_as(&anna, &seek(v)).is_err(), "seek {:?}", &v[..v.len().min(20)]);
    }
    // Every Variant Bragd plays fits.
    for info in bragd_games::VARIANTS {
        db.sql_as(&anna, &challenge(info.key)).unwrap_or_else(|e| panic!("{}: {e}", info.key));
        db.sql_as(&anna, &seek(info.key)).unwrap_or_else(|e| panic!("{}: {e}", info.key));
        db.sql_as(&anna, "delete from public.seeks").unwrap();
        db.owner().execute("delete from public.games", &[]).unwrap();
    }
}

/// A challenge or a seek written past the game service, in a Variant its rules do not know.
fn an_unplayable_variant_is_refused_cleanly(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let row = db
        .sql_as(&anna, &format!("insert into public.games (first, second, challenger, variant) values ('{0}', '{1}', '{0}', 'nosuch') returning id", anna.id, bo.id))
        .unwrap();
    let id: uuid::Uuid = row[0].get(0);
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})), (409, json!({ "error": "variant" })));
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/decline"), json!({})).0, 200, "it can still be declined");
    let row = db.sql_as(&anna, &format!("insert into public.seeks (player, variant) values ('{}', 'nosuch') returning id", anna.id)).unwrap();
    let id: uuid::Uuid = row[0].get(0);
    assert_eq!(db.call(&bo, "POST", &format!("/v1/seeks/{id}/accept"), json!({})), (409, json!({ "error": "variant" })));
}

fn count(db: &Db, sql: &str) -> i64 {
    db.owner().query_one(sql, &[]).unwrap().get(0)
}

fn writes_are_capped_for_each_player(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let carl = db.user("Carl");
    // Straight through Supabase's API: each table stops taking a player's rows at its cap.
    let fill = |sql: &dyn Fn(usize) -> String| (0..40).take_while(|i| db.sql_as(&anna, &sql(*i)).is_ok()).count();
    assert_eq!(fill(&|i| format!("insert into public.reports (reported, reason) values ('{}', 'r{i}')", bo.id)), 20, "reports a day");
    assert_eq!(fill(&|i| format!("insert into public.invites (code, inviter) values ('code-{i:08}', '{}')", anna.id)), 20, "invites");
    assert_eq!(fill(&|i| format!("insert into public.clubs (name, code, created_by) values ('C', 'club-{i:08}', '{}')", anna.id)), 5, "Clubs a day");
    let challenge = format!("insert into public.games (first, second, challenger, variant) values ('{0}', '{1}', '{0}', 'chess')", anna.id, carl.id);
    assert_eq!(fill(&|_| challenge.clone()), 10, "open challenges");
    // The game service says why.
    assert_eq!(db.call(&anna, "POST", "/v1/reports", json!({ "name": "Bo", "reason": "again" })), (429, json!({ "error": "too_often" })));
    assert_eq!(db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess" })), (429, json!({ "error": "too_often" })));
    // Another player's quota is their own.
    assert_eq!(db.call(&carl, "POST", "/v1/reports", json!({ "name": "Bo", "reason": "rude" })).0, 200);
    // Requests are limited for each caller, so a loop cannot hold up everyone else.
    let mut db = db;
    db.service.limits = bragd_server::api::Limits::default();
    let codes: Vec<u16> = (0..300).map(|_| db.call(&bo, "GET", "/v1/config", json!({})).0).collect();
    assert!(!codes.contains(&429), "config needs no sign-in and is not counted");
    let refused = (0..300).filter(|_| db.call(&bo, "POST", "/v1/nothing", json!({})).0 == 429).count();
    assert!(refused > 0, "three hundred requests at once");
    assert_eq!(db.call(&carl, "POST", "/v1/nothing", json!({})).0, 404, "Carl is not held up by Bo");
}

fn challenges_cannot_push_games_under_way_out(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess" }));
    let real = v["id"].as_str().unwrap().to_string();
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{real}/accept"), json!({})).0, 200);
    // Sixty challengers, as other accounts would, each with a challenge newer than the Game.
    for i in 0..60 {
        let c = db.user(&format!("Spam{i}"));
        assert_eq!(db.call(&c, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess" })).0, 200);
    }
    let games = db.call(&bo, "GET", "/v1/state", json!({})).1["games"].clone();
    assert!(games.as_array().unwrap().iter().any(|g| g["id"] == real.as_str()), "Bo's Game under way is listed");
}

fn old_challenges_and_invites_expire(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess" }));
    let id = uuid::Uuid::parse_str(v["id"].as_str().unwrap()).unwrap();
    let code = db.call(&anna, "POST", "/v1/invites", json!({})).1["code"].as_str().unwrap().to_string();
    let mut o = db.owner();
    o.execute("update public.games set created_at = now() - interval '8 days', updated_at = now() - interval '8 days' where id = $1", &[&id]).unwrap();
    o.execute("update public.invites set created_at = now() - interval '8 days'", &[]).unwrap();
    assert_eq!(db.call(&bo, "GET", "/v1/state", json!({})).1["games"], json!([]));
    let status: String = o.query_one("select status from public.games where id = $1", &[&id]).unwrap().get(0);
    assert_eq!(status, "cancelled");
    assert_eq!(db.call(&bo, "POST", "/v1/invites/redeem", json!({ "code": code })), (404, json!({ "error": "no_invite" })));
    // A new invite clears the inviter's expired ones.
    db.call(&anna, "POST", "/v1/invites", json!({}));
    assert_eq!(count(&db, "select count(*) from public.invites"), 1);
}

/// A client that announces a body and never sends it: the service answers others meanwhile,
/// and drops it at its deadline.
fn a_stalled_request_holds_up_nobody(pg: &Pg) {
    let db = pg.fresh();
    let addr = bragd_server::http::start_with(db.service, "127.0.0.1:0", 1, Duration::from_secs(1)).unwrap();
    let mut stalled = TcpStream::connect(&addr).unwrap();
    stalled.write_all(b"PUT /v1/me HTTP/1.1\r\nHost: x\r\nContent-Length: 1000\r\n\r\n0123456789").unwrap();
    // Guards against a hang only.
    stalled.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(20))).build().into();
    let r = agent.get(&format!("http://{addr}/v1/config")).call().expect("answered while another request stalls");
    assert_eq!(r.status(), 200);
    let mut rest = vec![];
    let closed = stalled.read_to_end(&mut rest);
    assert!(closed.is_ok() || closed.as_ref().is_err_and(|e| e.kind() == std::io::ErrorKind::ConnectionReset), "{closed:?}");
}

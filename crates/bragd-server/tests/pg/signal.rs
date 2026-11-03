//! Call signalling: a private Realtime topic per pair, open to those two only while Friends.

use serde_json::json;

use crate::support::{Db, Pg, User};

pub const CASES: &[crate::Case] = &[
    ("signals_pass_between_friends_only", signals_pass_between_friends_only),
    ("turn_credentials_are_the_callers_own", turn_credentials_are_the_callers_own),
    ("a_games_changes_are_told_to_its_two_players_only", a_games_changes_are_told_to_its_two_players_only),
];

fn turn_credentials_are_the_callers_own(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let (status, v) = db.call(&anna, "GET", "/v1/turn", json!({}));
    assert_eq!(status, 200);
    let servers = v["ice_servers"].as_array().unwrap();
    assert_eq!(servers[0], json!({ "urls": ["stun:turn.test:3478"] }));
    assert_eq!(servers[1]["urls"], json!(["turn:turn.test:3478", "turns:turn.test:5349"]));
    assert!(servers[1]["username"].as_str().unwrap().ends_with(&format!(":{}", anna.id)));
    assert!(!v.to_string().contains("turn-secret"), "the secret never leaves");
    assert_eq!(db.service.handle("GET", "/v1/turn", None, "").0, 401, "signed-in players only");
}

fn topic(a: &User, b: &User) -> String {
    let (x, y) = (a.id.to_string(), b.id.to_string());
    if x < y { format!("call:{x}:{y}") } else { format!("call:{y}:{x}") }
}

/// Whether `u` may join `topic` (Realtime reads the messages policy) and send on it.
fn may(db: &Db, u: &User, topic: &str) -> (bool, bool) {
    let read = db.sql_on(u, topic, "select 1 from realtime.messages").is_ok_and(|_| {
        // A row only the policy can hide: Realtime's join check reads the topic's messages.
        let mut c = db.owner();
        c.execute("insert into realtime.messages (topic, extension) values ($1, 'broadcast')", &[&topic]).unwrap();
        let n = db.sql_on(u, topic, "select 1 from realtime.messages").unwrap().len();
        c.execute("delete from realtime.messages", &[]).unwrap();
        n > 0
    });
    let sql = format!("insert into realtime.messages (topic, extension, event) values ('{topic}', 'broadcast', 'signal')");
    (read, db.sql_on(u, topic, &sql).is_ok())
}

fn signals_pass_between_friends_only(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let eve = db.user("Eve");
    let t = topic(&anna, &bo);
    assert_eq!(may(&db, &anna, &t), (false, false), "not Friends yet");
    db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" }));
    db.call(&bo, "POST", "/v1/friends", json!({ "name": "Anna" }));
    assert_eq!(may(&db, &anna, &t), (true, true));
    assert_eq!(may(&db, &bo, &t), (true, true));
    assert_eq!(may(&db, &eve, &t), (false, false), "a third player never joins");
    let (x, y) = t.trim_start_matches("call:").split_once(':').unwrap();
    assert_eq!(may(&db, &anna, &format!("call:{y}:{x}")), (false, false), "only the topic in order");
    assert_eq!(may(&db, &anna, "lobby"), (false, false), "no other topic");
    db.call(&bo, "POST", "/v1/blocks", json!({ "name": "Anna" }));
    assert_eq!(may(&db, &anna, &t), (false, false), "a block ends it");
}

/// The Games each player is told of on their own topic, oldest first.
fn told(db: &Db, u: &User) -> Vec<String> {
    let rows = db.owner().query("select payload from realtime.messages where topic = $1 order by id", &[&format!("player:{}", u.id)]).unwrap();
    rows.iter().map(|r| r.get::<_, serde_json::Value>(0)).map(|p| { assert_eq!(p["from"], "bragd"); p["signal"]["game"].as_str().unwrap().to_string() }).collect()
}

fn a_games_changes_are_told_to_its_two_players_only(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let eve = db.user("Eve");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess", "side": "first" }));
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(told(&db, &bo), std::slice::from_ref(&id), "the challenge");
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
    let before = told(&db, &bo).len();
    assert_eq!(db.call(&anna, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": 0, "entry": "e2e4" })).0, 200);
    assert!(told(&db, &bo).len() > before, "a move without a clock is told too");
    assert_eq!(told(&db, &anna).last(), Some(&id));
    assert!(told(&db, &eve).is_empty());
    let mine = format!("player:{}", anna.id);
    assert_eq!(may(&db, &anna, &mine), (true, false), "Anna listens on her own topic and never sends on it");
    assert_eq!(may(&db, &bo, &mine), (false, false), "nobody else listens there");
}

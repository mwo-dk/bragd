//! Chat in a Game: both say yes, either closes it, and it is kept only when both say so.

use serde_json::json;

use crate::support::{Db, Pg, User};

pub const CASES: &[crate::Case] = &[
    ("chat_opens_only_when_both_agree", chat_opens_only_when_both_agree),
    ("chat_either_closes_it_for_both", chat_either_closes_it_for_both),
    ("chat_is_forgotten_unless_both_keep_it", chat_is_forgotten_unless_both_keep_it),
    ("chat_is_kept_when_both_say_so", chat_is_kept_when_both_say_so),
];

fn game(db: &Db) -> (User, User, String) {
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess", "side": "first" }));
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
    (anna, bo, id)
}

fn chat(db: &Db, u: &User) -> serde_json::Value {
    db.call(u, "GET", "/v1/state", json!({})).1["games"][0]["chat"].clone()
}

fn chat_opens_only_when_both_agree(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = game(&db);
    let act = |u, what: &str, body| db.call(u, "POST", &format!("/v1/games/{id}/{what}"), body);
    assert_eq!(act(&anna, "say", json!({ "text": "hi" })), (409, json!({ "error": "chat_closed" })), "off by default");
    assert_eq!(act(&anna, "chat", json!({ "on": true })).0, 200);
    assert_eq!((chat(&db, &bo)["mine"].as_bool(), chat(&db, &bo)["theirs"].as_bool()), (Some(false), Some(true)), "Bo is asked");
    assert_eq!(act(&anna, "say", json!({ "text": "hi" })).0, 409, "not while only one agrees");
    let sql = format!("insert into public.chat (game_id, text) values ('{id}', 'sneaky')");
    assert!(db.sql_as(&anna, &sql).is_err(), "nor through Supabase's own API");
    assert_eq!(act(&bo, "chat", json!({ "on": true })).0, 200);
    assert_eq!(act(&anna, "say", json!({ "text": "Good luck!" })).0, 200);
    assert_eq!(chat(&db, &bo)["lines"], json!([{ "mine": false, "text": "Good luck!" }]));
    assert_eq!(chat(&db, &anna)["lines"], json!([{ "mine": true, "text": "Good luck!" }]));
    let eve = db.user("Eve");
    assert!(db.sql_as(&eve, "select * from public.chat").unwrap().is_empty(), "only the two read it");
    assert_eq!(db.call(&anna, "GET", "/v1/state", json!({})).1["games"][0]["friend"], false, "a stranger may Chat");
}

fn chat_either_closes_it_for_both(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = game(&db);
    let act = |u, what: &str, body| db.call(u, "POST", &format!("/v1/games/{id}/{what}"), body);
    act(&anna, "chat", json!({ "on": true }));
    act(&bo, "chat", json!({ "on": true }));
    assert_eq!(act(&bo, "chat", json!({ "on": false })).0, 200);
    let c = chat(&db, &anna);
    assert_eq!((c["mine"].as_bool(), c["theirs"].as_bool()), (Some(false), Some(false)));
    assert_eq!(act(&anna, "say", json!({ "text": "hello?" })).0, 409);
}

fn chat_is_forgotten_unless_both_keep_it(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = game(&db);
    let act = |u, what: &str, body| db.call(u, "POST", &format!("/v1/games/{id}/{what}"), body);
    act(&anna, "chat", json!({ "on": true }));
    act(&bo, "chat", json!({ "on": true }));
    act(&anna, "say", json!({ "text": "gg" }));
    act(&anna, "keep", json!({ "on": true }));
    act(&anna, "resign", json!({}));
    let n: i64 = db.owner().query_one("select count(*) from public.chat", &[]).unwrap().get(0);
    assert_eq!(n, 0, "only Anna wanted it kept");
    assert_eq!(act(&anna, "say", json!({ "text": "late" })).0, 409, "over");
}

fn chat_is_kept_when_both_say_so(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = game(&db);
    let act = |u, what: &str, body| db.call(u, "POST", &format!("/v1/games/{id}/{what}"), body);
    act(&anna, "chat", json!({ "on": true }));
    act(&bo, "chat", json!({ "on": true }));
    act(&bo, "say", json!({ "text": "well played" }));
    act(&anna, "keep", json!({ "on": true }));
    act(&bo, "keep", json!({ "on": true }));
    assert_eq!(chat(&db, &anna)["keep_theirs"], true);
    act(&bo, "resign", json!({}));
    assert_eq!(chat(&db, &anna)["lines"], json!([{ "mine": false, "text": "well played" }]));
}

//! Deleting an account: nothing with its id is left, and the other players' Games keep a
//! deleted player when the user agreed.

use serde_json::{Value, json};

use crate::support::{Db, Pg, User};

pub const CASES: &[crate::Case] = &[
    ("deleting_an_account_leaves_no_row_with_its_id", deleting_an_account_leaves_no_row_with_its_id),
    ("deleting_without_keeping_takes_the_games_too", deleting_without_keeping_takes_the_games_too),
];

/// Anna with everything an account can have: a Friend, an invite, a block, reports both ways, a
/// Club with a member, a Game that ended, one under way with her Chat line, an open challenge,
/// and a Call signal.
fn busy_anna(db: &Db) -> (User, User, User) {
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let carl = db.user("Carl");
    db.user("Dan");
    db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" }));
    db.call(&bo, "POST", "/v1/friends", json!({ "name": "Anna" }));
    db.call(&anna, "POST", "/v1/invites", json!({}));
    db.call(&anna, "POST", "/v1/blocks", json!({ "name": "Dan" }));
    db.call(&anna, "POST", "/v1/reports", json!({ "name": "Carl", "reason": "rude" }));
    db.call(&carl, "POST", "/v1/reports", json!({ "name": "Anna", "reason": "rude too" }));
    let club = db.call(&anna, "POST", "/v1/clubs", json!({ "name": "Skakklubben" })).1;
    db.call(&bo, "POST", "/v1/clubs/join", json!({ "code": club["code"] }));
    db.call(&anna, "POST", "/v1/clubs", json!({ "name": "Annas egen" }));
    let game = |them: &User, accept: bool| {
        let id = db.call(&anna, "POST", "/v1/games", json!({ "opponent": name(db, them), "variant": "chess", "side": "first" })).1["id"].as_str().unwrap().to_string();
        if accept {
            assert_eq!(db.call(them, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
        }
        id
    };
    let ended = game(&bo, true);
    db.call(&anna, "POST", &format!("/v1/games/{ended}/move"), json!({ "ply": 0, "entry": "e2e4" }));
    db.call(&anna, "POST", &format!("/v1/games/{ended}/draw"), json!({ "yes": true }));
    db.call(&bo, "POST", &format!("/v1/games/{ended}/resign"), json!({}));
    let playing = game(&carl, true);
    for u in [&anna, &carl] {
        db.call(u, "POST", &format!("/v1/games/{playing}/chat"), json!({ "on": true }));
        db.call(u, "POST", &format!("/v1/games/{playing}/keep"), json!({ "on": true }));
    }
    assert_eq!(db.call(&anna, "POST", &format!("/v1/games/{playing}/say"), json!({ "text": "hi" })).0, 200);
    assert_eq!(db.call(&carl, "POST", &format!("/v1/games/{playing}/say"), json!({ "text": "hello" })).0, 200);
    game(&bo, false);
    let (a, b) = (anna.id.min(bo.id), anna.id.max(bo.id));
    db.owner().execute("insert into realtime.messages (topic, extension) values ($1, 'broadcast')", &[&format!("call:{a}:{b}")]).unwrap();
    (anna, bo, carl)
}

fn name(db: &Db, u: &User) -> String {
    db.call(u, "GET", "/v1/state", json!({})).1["me"].as_str().unwrap().to_string()
}

/// Every table, in every schema, whose row mentions `id` in any column.
fn rows_with(db: &Db, id: uuid::Uuid) -> Vec<String> {
    let mut c = db.owner();
    let tables: Vec<String> = c
        .query(
            "select format('%I.%I', table_schema, table_name) from information_schema.tables \
             where table_type = 'BASE TABLE' and table_schema not in ('pg_catalog', 'information_schema')",
            &[],
        )
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert!(tables.iter().any(|t| t == "auth.users"), "{tables:?}");
    tables
        .into_iter()
        .filter(|t| {
            let n: i64 = c.query_one(&format!("select count(*) from {t} x where x::text like '%' || $1 || '%'"), &[&id.to_string()]).unwrap().get(0);
            n > 0
        })
        .collect()
}

fn delete(db: &Db, u: &User, keep: bool) -> (u16, Value) {
    db.service.handle("DELETE", "/v1/me", Some(&format!("Bearer {}", u.token)), &json!({ "keep_games": keep }).to_string())
}

fn deleting_an_account_leaves_no_row_with_its_id(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, carl) = busy_anna(&db);
    assert!(rows_with(&db, anna.id).len() >= 10, "the setup touched every table");
    assert!(db.sql_as(&bo, &format!("select bragd.forget('{}', false)", carl.id)).is_err(), "nobody deletes another's account");
    assert_eq!(delete(&db, &anna, true), (200, json!({})));
    assert_eq!(rows_with(&db, anna.id), Vec::<String>::new());
    assert!(!rows_with(&db, bo.id).is_empty(), "Bo's account stays");

    // Bo keeps the Game that ended, against a deleted player; the open challenge is gone.
    let st = db.call(&bo, "GET", "/v1/state", json!({})).1;
    let games = st["games"].as_array().unwrap();
    assert_eq!(games.len(), 1, "{st}");
    assert_eq!(games[0]["opponent"], Value::Null);
    assert_eq!(games[0]["opponent_id"], Value::Null);
    assert_eq!(games[0]["moves"], json!(["e2e4"]));
    assert_eq!(games[0]["result"], "1-0");
    assert_eq!(st["friends"], json!([]));
    let clubs = &st["clubs"];
    assert_eq!((clubs.as_array().unwrap().len(), &clubs[0]["name"], &clubs[0]["members"]), (1, &json!("Skakklubben"), &json!(1)), "{st}");
    assert_ne!(clubs[0]["code"], "", "the Club Anna made is Bo's now");
    let alone: i64 = db.owner().query_one("select count(*) from public.clubs where name = 'Annas egen'", &[]).unwrap().get(0);
    assert_eq!(alone, 0, "a Club with no other member goes");
    let id = games[0]["id"].as_str().unwrap();
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/resign"), json!({})), (409, json!({ "error": "not_now" })));

    // Carl's Game under way ended as Anna's resignation; the Chat both kept stays whole.
    let st = db.call(&carl, "GET", "/v1/state", json!({})).1;
    let g = &st["games"][0];
    assert_eq!((g["status"].as_str(), g["result"].as_str(), g["reason"].as_str()), (Some("over"), Some("0-1"), Some("resign")), "{st}");
    assert_eq!(g["chat"]["lines"], json!([{ "mine": false, "text": "hi" }, { "mine": true, "text": "hello" }]));
    assert_reports_kept(&db);

    // Her token no longer opens anything of hers, and deleting again is harmless.
    assert_eq!(db.call(&anna, "GET", "/v1/state", json!({})).1["me"], Value::Null);
    assert_eq!(delete(&db, &anna, true).0, 200);
}

fn deleting_without_keeping_takes_the_games_too(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, carl) = busy_anna(&db);
    assert_eq!(delete(&db, &anna, false), (200, json!({})));
    assert_eq!(rows_with(&db, anna.id), Vec::<String>::new());
    for u in [&bo, &carl] {
        assert_eq!(db.call(u, "GET", "/v1/state", json!({})).1["games"], json!([]));
    }
    let n: i64 = db.owner().query_one("select count(*) from public.games", &[]).unwrap().get(0);
    assert_eq!(n, 0);
    assert_reports_kept(&db);
}

/// The report Carl made about Anna is still the operator's, with her name and without her id;
/// the one she made is gone with her.
fn assert_reports_kept(db: &Db) {
    let rows = db.owner().query("select reported is null, reported_name, reason from public.reports", &[]).unwrap();
    let rows: Vec<(bool, Option<String>, String)> = rows.iter().map(|r| (r.get(0), r.get(1), r.get(2))).collect();
    assert_eq!(rows, vec![(true, Some("Anna".to_string()), "rude too".to_string())]);
}

//! Clocks, ratings and seeks: the referee keeps the time, rates the Game and pairs a seek.

use serde_json::json;

use crate::support::{Db, Pg, User};

pub const CASES: &[crate::Case] = &[
    ("a_clock_is_charged_on_the_server_and_its_flag_falls_there", a_clock_is_charged_on_the_server_and_its_flag_falls_there),
    ("a_flag_is_claimed_only_once_the_time_is_up", a_flag_is_claimed_only_once_the_time_is_up),
    ("players_cannot_set_a_clock_or_a_rating_themselves", players_cannot_set_a_clock_or_a_rating_themselves),
    ("a_rated_game_changes_both_ratings_and_a_casual_one_none", a_rated_game_changes_both_ratings_and_a_casual_one_none),
    ("a_seek_is_taken_by_another_player_once", a_seek_is_taken_by_another_player_once),
    ("a_seek_is_hidden_between_players_who_block", a_seek_is_hidden_between_players_who_block),
    ("a_seek_is_the_players_own", a_seek_is_the_players_own),
];

/// Anna (first) and Bo playing chess with `body`'s time control.
fn playing(db: &Db, body: serde_json::Value) -> (User, User, String) {
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let mut b = json!({ "opponent": "Bo", "variant": "chess", "side": "first" });
    b.as_object_mut().unwrap().extend(body.as_object().unwrap().clone());
    let (s, v) = db.call(&anna, "POST", "/v1/games", b);
    assert_eq!(s, 200, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
    (anna, bo, id)
}

fn game(db: &Db, u: &User) -> serde_json::Value {
    db.call(u, "GET", "/v1/state", json!({})).1["games"][0].clone()
}

/// Moves the running clock's start back by `secs`, as if that much time had passed.
fn wait(db: &Db, id: &str, secs: i32) {
    db.owner().execute(&format!("update public.games set turn_started = turn_started - make_interval(secs => {secs}) where id = '{id}'"), &[]).unwrap();
}

fn mv(db: &Db, u: &User, id: &str, ply: u64, e: &str) -> (u16, serde_json::Value) {
    db.call(u, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": ply, "entry": e }))
}

fn a_clock_is_charged_on_the_server_and_its_flag_falls_there(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = playing(&db, json!({ "minutes": 1, "increment": 2 }));
    let g = game(&db, &bo);
    assert_eq!(g["clock"]["running"], 0);
    assert_eq!(g["clock"]["increment"], 2000);
    assert!(g["clock"]["left"][0].as_i64().unwrap() <= 60000);
    assert_eq!(mv(&db, &anna, &id, 0, "e2e4").0, 200);
    wait(&db, &id, 30);
    assert_eq!(mv(&db, &bo, &id, 1, "e7e5").0, 200);
    let left = game(&db, &anna)["clock"]["left"].clone();
    let bo_left = left[1].as_i64().unwrap();
    assert!((31000..=32000).contains(&bo_left), "30 s used, 2 s added: {left}");
    assert!(left[0].as_i64().unwrap() > 60000, "Anna's increment: {left}");
    // Anna's flag falls while she thinks: her move comes too late, and the Game is Bo's.
    wait(&db, &id, 120);
    let (s, v) = mv(&db, &anna, &id, 2, "g1f3");
    assert_eq!((s, &v["status"]), (200, &json!("over")), "{v}");
    let g = game(&db, &anna);
    assert_eq!((g["result"].as_str(), g["reason"].as_str()), (Some("0-1"), Some("time")));
    assert_eq!(g["moves"].as_array().unwrap().len(), 2, "the late move is not played");
}

fn a_flag_is_claimed_only_once_the_time_is_up(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = playing(&db, json!({ "minutes": 1 }));
    let flag = |u| db.call(u, "POST", &format!("/v1/games/{id}/flag"), json!({}));
    assert_eq!(flag(&bo), (409, json!({ "error": "not_now" })));
    wait(&db, &id, 61);
    assert_eq!(flag(&bo).0, 200);
    let g = game(&db, &anna);
    assert_eq!((g["result"].as_str(), g["reason"].as_str()), (Some("0-1"), Some("time")));
    // No clock, no flag.
    let db = pg.fresh();
    let (_anna, bo, id) = playing(&db, json!({}));
    assert_eq!(game(&db, &bo)["clock"], json!(null));
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/flag"), json!({})), (409, json!({ "error": "not_now" })));
}

fn players_cannot_set_a_clock_or_a_rating_themselves(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = playing(&db, json!({ "minutes": 5, "rated": true }));
    assert!(db.sql_as(&anna, &format!("update public.games set left_first = 999999 where id = '{id}'")).is_err());
    let sql = format!("insert into public.games (first, second, challenger, variant, left_first) values ('{0}', '{1}', '{0}', 'chess', 999999)", anna.id, bo.id);
    assert!(db.sql_as(&anna, &sql).is_err());
    assert!(db.sql_as(&anna, &format!("insert into public.ratings (player, family, rating) values ('{}', 'chess', 3000)", anna.id)).is_err());
    assert!(db.sql_as(&anna, "update public.ratings set rating = 3000").is_err());
    let (s, _) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess", "minutes": 0 }));
    assert_eq!(s, 400, "a clock of no time");
}

fn a_rated_game_changes_both_ratings_and_a_casual_one_none(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = playing(&db, json!({ "rated": true }));
    assert_eq!(game(&db, &anna)["rated"], true);
    assert_eq!(db.call(&anna, "POST", &format!("/v1/games/{id}/resign"), json!({})).0, 200);
    let ratings = |u| db.call(u, "GET", "/v1/state", json!({})).1["ratings"].clone();
    let (a, b) = (ratings(&anna), ratings(&bo));
    assert_eq!(a[0]["family"], "chess");
    assert_eq!(a[0]["games"], 1);
    assert!(a[0]["rating"].as_f64().unwrap() < 1500.0 && b[0]["rating"].as_f64().unwrap() > 1500.0, "{a} {b}");
    assert!(a[0]["deviation"].as_f64().unwrap() < 350.0);
    // A casual Game leaves them as they are.
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess" }));
    let id = v["id"].as_str().unwrap().to_string();
    db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({}));
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/resign"), json!({})).0, 200);
    assert_eq!(ratings(&anna), a);
}

fn a_seek_is_taken_by_another_player_once(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let carl = db.user("Carl");
    let (s, v) = db.call(&anna, "POST", "/v1/seeks", json!({ "variant": "ur", "minutes": 5, "increment": 3, "rated": true }));
    assert_eq!(s, 200, "{v}");
    let seeks = |u| db.call(u, "GET", "/v1/state", json!({})).1["seeks"].clone();
    assert_eq!(seeks(&anna)[0]["mine"], true);
    let seen = seeks(&bo);
    assert_eq!((seen[0]["name"].as_str(), seen[0]["variant"].as_str(), seen[0]["mine"].as_bool()), (Some("Anna"), Some("ur"), Some(false)));
    assert_eq!((seen[0]["minutes"].as_i64(), seen[0]["increment"].as_i64(), seen[0]["rated"].as_bool()), (Some(5), Some(3), Some(true)));
    let sid = seen[0]["id"].as_str().unwrap().to_string();
    assert_eq!(db.call(&anna, "POST", &format!("/v1/seeks/{sid}/accept"), json!({})), (409, json!({ "error": "own_seek" })));
    let (s, v) = db.call(&bo, "POST", &format!("/v1/seeks/{sid}/accept"), json!({}));
    assert_eq!(s, 200, "{v}");
    let gid = v["id"].as_str().unwrap();
    let g = game(&db, &anna);
    assert_eq!((g["id"].as_str(), g["status"].as_str(), g["rated"].as_bool()), (Some(gid), Some("playing"), Some(true)));
    assert_eq!(g["clock"]["increment"], 3000);
    assert_eq!(seeks(&anna), json!([]));
    assert_eq!(db.call(&carl, "POST", &format!("/v1/seeks/{sid}/accept"), json!({})), (404, json!({ "error": "no_seek" })));
    // Nonsense is refused.
    assert_eq!(db.call(&carl, "POST", "/v1/seeks", json!({ "variant": "nonsense" })).0, 400);
    assert_eq!(db.call(&carl, "POST", "/v1/seeks", json!({ "variant": "chess", "minutes": 999 })).0, 400);
}

fn a_seek_is_hidden_between_players_who_block(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let eve = db.user("Eve");
    db.call(&anna, "POST", "/v1/seeks", json!({ "variant": "chess" }));
    let sid = db.call(&eve, "GET", "/v1/state", json!({})).1["seeks"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(db.call(&anna, "POST", "/v1/blocks", json!({ "name": "Eve" })).0, 200);
    assert_eq!(db.call(&eve, "GET", "/v1/state", json!({})).1["seeks"], json!([]));
    assert_eq!(db.call(&eve, "POST", &format!("/v1/seeks/{sid}/accept"), json!({})), (404, json!({ "error": "no_seek" })));
}

fn a_seek_is_the_players_own(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    db.call(&anna, "POST", "/v1/seeks", json!({ "variant": "chess" }));
    // A new seek replaces the old one.
    assert_eq!(db.call(&anna, "POST", "/v1/seeks", json!({ "variant": "shogi" })).0, 200);
    let seeks = db.call(&bo, "GET", "/v1/state", json!({})).1["seeks"].clone();
    assert_eq!(seeks.as_array().unwrap().len(), 1);
    assert_eq!(seeks[0]["variant"], "shogi");
    assert!(db.sql_as(&bo, "delete from public.seeks returning 1").unwrap().is_empty(), "Bo cannot take Anna's seek back");
    assert!(db.sql_as(&bo, &format!("insert into public.seeks (player, variant) values ('{}', 'chess')", anna.id)).is_err());
    assert_eq!(db.call(&anna, "DELETE", "/v1/seeks", json!({})).0, 200);
    assert_eq!(db.call(&bo, "GET", "/v1/state", json!({})).1["seeks"], json!([]));
}

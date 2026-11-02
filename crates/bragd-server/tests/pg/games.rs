//! Profiles, challenges and the referee.

use serde_json::json;

use crate::support::Pg;

pub const CASES: &[crate::Case] = &[
    ("config_needs_no_sign_in", config_needs_no_sign_in),
    ("a_token_from_elsewhere_is_refused", a_token_from_elsewhere_is_refused),
    ("names_are_unique_and_your_own", names_are_unique_and_your_own),
    ("a_challenge_is_accepted_and_played_to_mate", a_challenge_is_accepted_and_played_to_mate),
    ("the_referee_refuses_wrong_moves", the_referee_refuses_wrong_moves),
    ("only_the_challenged_accept_and_only_the_challenger_cancels", only_the_challenged_accept_and_only_the_challenger_cancels),
    ("a_game_is_seen_by_its_players_only", a_game_is_seen_by_its_players_only),
    ("players_cannot_write_moves_or_results_themselves", players_cannot_write_moves_or_results_themselves),
    ("resign_and_draw_by_agreement", resign_and_draw_by_agreement),
    ("dice_families_are_refused", dice_families_are_refused),
    ("the_api_over_http", the_api_over_http),
];

fn config_needs_no_sign_in(pg: &Pg) {
    let db = pg.fresh();
    let (s, v) = db.service.handle("GET", "/v1/config", None, "");
    assert_eq!(s, 200);
    assert_eq!(v["oauth"], json!(["github"]));
    assert_eq!(db.service.handle("GET", "/v1/state", None, "").0, 401);
}

fn a_token_from_elsewhere_is_refused(pg: &Pg) {
    let db = pg.fresh();
    let mut u = db.account();
    u.token = bragd_server::jwt::sign_hs256("another-project", &bragd_server::jwt::user_claims(u.id, 4_000_000_000));
    assert_eq!(db.call(&u, "GET", "/v1/state", json!({})).0, 401);
}

fn names_are_unique_and_your_own(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.account();
    assert_eq!(db.call(&bo, "PUT", "/v1/me", json!({ "name": "Anna" })), (409, json!({ "error": "taken" })));
    assert_eq!(db.call(&bo, "PUT", "/v1/me", json!({ "name": "B" })).0, 400, "too short");
    assert_eq!(db.call(&bo, "PUT", "/v1/me", json!({ "name": "Bo" })).0, 200);
    // Through Supabase's own API, a player cannot write another's profile.
    let n = db.sql_as(&bo, &format!("update public.profiles set name = 'Mallory' where id = '{}' returning 1", anna.id)).unwrap();
    assert!(n.is_empty());
    assert!(db.sql_as(&bo, &format!("insert into public.profiles (id, name) values ('{}', 'Eve')", uuid::Uuid::new_v4())).is_err());
    let (_, v) = db.call(&anna, "GET", "/v1/state", json!({}));
    assert_eq!(v["me"], "Anna");
}

/// Anna challenges Bo, Bo accepts, and the fool's mate ends it.
fn a_challenge_is_accepted_and_played_to_mate(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (s, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess", "side": "second" }));
    assert_eq!(s, 200, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let (_, st) = db.call(&bo, "GET", "/v1/state", json!({}));
    assert_eq!(st["games"][0]["status"], "offered");
    assert_eq!(st["games"][0]["opponent"], "Anna");
    assert_eq!(st["games"][0]["first"], true);
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
    for (ply, (who, mv)) in [(&bo, "f2f3"), (&anna, "e7e5"), (&bo, "g2g4"), (&anna, "d8h4")].into_iter().enumerate() {
        let (s, v) = db.call(who, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": ply, "entry": mv }));
        assert_eq!(s, 200, "{mv}: {v}");
    }
    let (_, st) = db.call(&anna, "GET", "/v1/state", json!({}));
    let g = &st["games"][0];
    assert_eq!(g["status"], "over");
    assert_eq!(g["result"], "0-1");
    assert_eq!(g["moves"], json!(["f2f3", "e7e5", "g2g4", "d8h4"]));
}

fn playing(db: &crate::support::Db) -> (crate::support::User, crate::support::User, String) {
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess", "side": "first" }));
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
    (anna, bo, id)
}

fn the_referee_refuses_wrong_moves(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = playing(&db);
    let mv = |u, ply: u64, e: &str| db.call(u, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": ply, "entry": e }));
    assert_eq!(mv(&bo, 0, "e7e5"), (409, json!({ "error": "not_your_turn" })));
    assert_eq!(mv(&anna, 0, "e2e5"), (400, json!({ "error": "illegal" })));
    assert_eq!(mv(&anna, 3, "e2e4"), (409, json!({ "error": "stale" })));
    assert_eq!(mv(&anna, 0, "e2e4").0, 200);
    assert_eq!(mv(&anna, 1, "d2d4"), (409, json!({ "error": "not_your_turn" })));
    let eve = db.user("Eve");
    assert_eq!(mv(&eve, 1, "e7e5"), (404, json!({ "error": "no_game" })), "not her Game");
}

fn only_the_challenged_accept_and_only_the_challenger_cancels(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "shogi" }));
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(db.call(&anna, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 409);
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/cancel"), json!({})).0, 409);
    assert_eq!(db.call(&anna, "POST", &format!("/v1/games/{id}/cancel"), json!({})).0, 200);
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 409, "cancelled");
    assert_eq!(db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Nobody", "variant": "chess" })).0, 404);
    assert_eq!(db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Anna", "variant": "chess" })).0, 404, "not yourself");
}

fn a_game_is_seen_by_its_players_only(pg: &Pg) {
    let db = pg.fresh();
    let (_anna, _bo, id) = playing(&db);
    let eve = db.user("Eve");
    let (_, st) = db.call(&eve, "GET", "/v1/state", json!({}));
    assert_eq!(st["games"], json!([]));
    assert!(db.sql_as(&eve, &format!("select 1 from public.games where id = '{id}'")).unwrap().is_empty());
    assert!(db.sql_as(&eve, &format!("select 1 from public.moves where game_id = '{id}'")).unwrap().is_empty());
}

fn players_cannot_write_moves_or_results_themselves(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = playing(&db);
    assert!(db.sql_as(&anna, &format!("insert into public.moves (game_id, ply, entry) values ('{id}', 0, 'e2e4')")).is_err());
    assert!(db.sql_as(&anna, &format!("update public.games set status = 'over', result = '1-0' where id = '{id}'")).is_err());
    // A challenge made directly must still be the caller's own, and open.
    let sql = format!("insert into public.games (first, second, challenger, variant) values ('{0}', '{1}', '{1}', 'chess')", anna.id, bo.id);
    assert!(db.sql_as(&anna, &sql).is_err(), "Anna cannot challenge in Bo's name");
    let sql = format!("insert into public.games (first, second, challenger, variant, status) values ('{0}', '{1}', '{0}', 'chess', 'playing')", anna.id, bo.id);
    assert!(db.sql_as(&anna, &sql).is_err(), "a challenge starts offered");
}

fn resign_and_draw_by_agreement(pg: &Pg) {
    let db = pg.fresh();
    let (anna, bo, id) = playing(&db);
    let act = |u, what: &str, body| db.call(u, "POST", &format!("/v1/games/{id}/{what}"), body);
    assert_eq!(act(&anna, "draw", json!({ "yes": true })).0, 200);
    let (_, st) = db.call(&bo, "GET", "/v1/state", json!({}));
    assert_eq!(st["games"][0]["draw_offered"], true);
    assert_eq!(act(&bo, "draw", json!({ "yes": true })).0, 200);
    let (_, st) = db.call(&bo, "GET", "/v1/state", json!({}));
    assert_eq!((st["games"][0]["result"].as_str(), st["games"][0]["reason"].as_str()), (Some("½-½"), Some("agreement")));
    assert_eq!(act(&anna, "resign", json!({})).0, 409, "over");

    let db = pg.fresh();
    let (anna, bo, id) = playing(&db);
    let act = |u, what: &str| db.call(u, "POST", &format!("/v1/games/{id}/{what}"), json!({}));
    assert_eq!(act(&anna, "resign").0, 200);
    let (_, st) = db.call(&bo, "GET", "/v1/state", json!({}));
    assert_eq!(st["games"][0]["result"], "0-1");
}

fn dice_families_are_refused(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    db.user("Bo");
    assert_eq!(db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "backgammon" })), (400, json!({ "error": "variant" })));
    assert_eq!(db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "nonsense" })).0, 400);
}

fn the_api_over_http(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let addr = bragd_server::http::start(db.service, "127.0.0.1:0").unwrap();
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let mut r = agent.get(&format!("http://{addr}/v1/state")).header("Authorization", &format!("Bearer {}", anna.token)).call().unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.body_mut().read_json::<serde_json::Value>().unwrap()["me"], "Anna");
    let big = "x".repeat(bragd_server::api::MAX_BODY + 1);
    let r = agent.put(&format!("http://{addr}/v1/me")).header("Authorization", &format!("Bearer {}", anna.token)).send(&big).unwrap();
    assert_eq!(r.status(), 413);
    assert_eq!(agent.get(&format!("http://{addr}/v1/state")).call().unwrap().status(), 401);
}

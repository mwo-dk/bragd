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
    ("unknown_variants_are_refused", unknown_variants_are_refused),
    ("ur_and_senet_are_played_with_the_referees_throws", ur_and_senet_are_played_with_the_referees_throws),
    ("backgammon_opens_and_doubles_with_the_referees_dice", backgammon_opens_and_doubles_with_the_referees_dice),
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

fn unknown_variants_are_refused(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    db.user("Bo");
    assert_eq!(db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "nonsense" })), (400, json!({ "error": "variant" })));
}

/// The Game `id` as `u` sees it: its record replayed by Bragd's rules.
fn seen(db: &crate::support::Db, u: &crate::support::User, id: &str) -> (serde_json::Value, bragd_games::State) {
    let (_, st) = db.call(u, "GET", "/v1/state", json!({}));
    let g = st["games"].as_array().unwrap().iter().find(|g| g["id"] == id).unwrap().clone();
    let entries: Vec<String> = serde_json::from_value(g["moves"].clone()).unwrap();
    let state = bragd_games::replay(g["variant"].as_str().unwrap(), g["initial"].as_str(), &entries).unwrap();
    (g, state)
}

/// Each Game is played to its end, or 300 entries, by the first legal move. The players only
/// ever move: every throw in the record is the referee's, and no throw is left waiting.
fn ur_and_senet_are_played_with_the_referees_throws(pg: &Pg) {
    for variant in ["ur", "senet"] {
        let db = pg.fresh();
        let anna = db.user("Anna");
        let bo = db.user("Bo");
        let (s, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": variant, "side": "first" }));
        assert_eq!(s, 200, "{v}");
        let id = v["id"].as_str().unwrap().to_string();
        assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
        let mut moved = 0;
        loop {
            let (g, state) = seen(&db, &anna, &id);
            if g["status"] == "over" {
                assert!(state.outcome.is_some(), "{variant}: over by the rules");
                break;
            }
            assert_eq!(state.awaiting, bragd_games::Awaiting::Move, "{variant}: the referee threw");
            let who = if state.to_move.unwrap().index == 0 { &anna } else { &bo };
            let other = if std::ptr::eq(who, &anna) { &bo } else { &anna };
            let ply = state.history.len();
            assert_eq!(db.call(other, "POST", &format!("/v1/games/{id}/throw"), json!({})).0, 409, "a throw is not asked for");
            let (s, v) = db.call(who, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": ply, "entry": state.moves[0].wire }));
            assert_eq!(s, 200, "{variant}: {v}");
            moved += 1;
            if moved == 150 {
                break;
            }
        }
        let (g, _) = seen(&db, &anna, &id);
        let entries = g["moves"].as_array().unwrap().len();
        assert!(entries > moved, "{variant}: the record holds the referee's throws as well as {moved} moves");
    }
}

/// The opening throw seats whoever throws higher. Before a later throw the player may double,
/// so the referee waits for them to ask; a player cannot write the dice themselves.
fn backgammon_opens_and_doubles_with_the_referees_dice(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "backgammon", "side": "first" }));
    let id = v["id"].as_str().unwrap().to_string();
    // A player cannot choose who starts through Supabase's own API either.
    let sql = format!("insert into public.games (first, second, challenger, variant, initial) values ('{0}', '{1}', '{0}', 'backgammon', 'white')", anna.id, bo.id);
    assert!(db.sql_as(&anna, &sql).is_err());
    assert_eq!(db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({})).0, 200);
    let (g, state) = seen(&db, &anna, &id);
    assert!(matches!(g["initial"].as_str(), Some("white" | "black")), "{g}");
    assert_eq!(g["moves"].as_array().unwrap().len(), 1, "the opening roll");
    assert_eq!(state.awaiting, bragd_games::Awaiting::Move);
    let seat = |s: &bragd_games::State| if s.to_move.unwrap().index == 0 { (&anna, &bo) } else { (&bo, &anna) };
    let (who, _) = seat(&state);
    assert_eq!(db.call(who, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": 1, "entry": state.moves[0].wire })).0, 200);
    // The other player may double before throwing: the referee waits.
    let (_, state) = seen(&db, &anna, &id);
    assert_eq!(state.awaiting, bragd_games::Awaiting::Throw);
    assert!(state.moves.iter().any(|m| m.wire == "double"));
    let (who, other) = seat(&state);
    let ply = state.history.len();
    assert_eq!(db.call(who, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": ply, "entry": "6-6" })), (400, json!({ "error": "illegal" })), "no player throws");
    assert_eq!(db.call(other, "POST", &format!("/v1/games/{id}/throw"), json!({})), (409, json!({ "error": "not_your_turn" })));
    assert_eq!(db.call(who, "POST", &format!("/v1/games/{id}/throw"), json!({})).0, 200);
    let (g, state) = seen(&db, &anna, &id);
    assert_eq!(g["moves"].as_array().unwrap().len(), ply + 1, "one throw");
    assert!(state.awaiting == bragd_games::Awaiting::Move || state.passed.is_some());
    // Doubling, then taking: the taker owns the cube, so the doubler's throw is made at once.
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let (_, v) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "backgammon" }));
    let id = v["id"].as_str().unwrap().to_string();
    db.call(&bo, "POST", &format!("/v1/games/{id}/accept"), json!({}));
    let (g, state) = seen(&db, &anna, &id);
    // Anna's side is the challenge's random one.
    let anna_first = g["first"] == true;
    let seat = |s: &bragd_games::State| if (s.to_move.unwrap().index == 0) == anna_first { (&anna, &bo) } else { (&bo, &anna) };
    assert_eq!(db.call(seat(&state).0, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": 1, "entry": state.moves[0].wire })).0, 200);
    let (_, state) = seen(&db, &anna, &id);
    let (doubler, taker) = seat(&state);
    assert_eq!(db.call(doubler, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": 2, "entry": "double" })).0, 200);
    assert_eq!(db.call(taker, "POST", &format!("/v1/games/{id}/move"), json!({ "ply": 3, "entry": "take" })).0, 200);
    let (g, state) = seen(&db, &anna, &id);
    assert_eq!(g["moves"].as_array().unwrap().len(), 5, "the doubler's throw follows the take");
    assert!(state.awaiting == bragd_games::Awaiting::Move || state.passed.is_some());
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

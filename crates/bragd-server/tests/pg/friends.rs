//! Friends, invites, blocks, reports and Clubs.

use serde_json::json;

use crate::support::{Db, Pg, User};

pub const CASES: &[crate::Case] = &[
    ("a_request_is_accepted_by_the_one_asked", a_request_is_accepted_by_the_one_asked),
    ("an_invite_makes_friends_once", an_invite_makes_friends_once),
    ("a_block_ends_hides_and_refuses", a_block_ends_hides_and_refuses),
    ("a_report_reaches_the_operator_only", a_report_reaches_the_operator_only),
    ("club_members_are_friends_while_both_allow_it", club_members_are_friends_while_both_allow_it),
];

fn friends(db: &Db, u: &User) -> serde_json::Value {
    db.call(u, "GET", "/v1/state", json!({})).1
}

fn a_request_is_accepted_by_the_one_asked(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let eve = db.user("Eve");
    assert_eq!(db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" })), (200, json!({ "friends": false })));
    assert_eq!(friends(&db, &bo)["asked"], json!(["Anna"]));
    assert_eq!(friends(&db, &anna)["asking"], json!(["Bo"]));
    // Asking again does not accept your own request.
    assert_eq!(db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" })).1, json!({ "friends": false }));
    let (a, b) = (anna.id.min(bo.id), anna.id.max(bo.id));
    let accept = format!("update public.friendships set accepted = true where a = '{a}' and b = '{b}' returning 1");
    assert!(db.sql_as(&anna, &accept).unwrap().is_empty(), "the asker cannot accept");
    assert!(db.sql_as(&eve, &accept).unwrap().is_empty(), "nor can anyone else");
    assert_eq!(db.call(&bo, "POST", "/v1/friends", json!({ "name": "Anna" })).1, json!({ "friends": true }));
    assert_eq!(friends(&db, &anna)["friends"], json!([{ "name": "Bo", "club": false }]));
    assert_eq!(friends(&db, &bo)["friends"], json!([{ "name": "Anna", "club": false }]));
    assert_eq!(friends(&db, &eve)["friends"], json!([]));
    assert_eq!(db.call(&anna, "POST", "/v1/friends/remove", json!({ "name": "Bo" })).0, 200);
    assert_eq!(friends(&db, &bo)["friends"], json!([]));
}

fn an_invite_makes_friends_once(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let eve = db.user("Eve");
    let code = db.call(&anna, "POST", "/v1/invites", json!({})).1["code"].as_str().unwrap().to_string();
    assert_eq!(db.call(&anna, "POST", "/v1/invites/redeem", json!({ "code": code })), (409, json!({ "error": "own_invite" })));
    let code = db.call(&anna, "POST", "/v1/invites", json!({})).1["code"].as_str().unwrap().to_string();
    assert_eq!(db.call(&bo, "POST", "/v1/invites/redeem", json!({ "code": code })), (200, json!({ "name": "Anna" })));
    assert_eq!(db.call(&eve, "POST", "/v1/invites/redeem", json!({ "code": code })).0, 404, "used once");
    assert_eq!(friends(&db, &bo)["friends"][0]["name"], "Anna");
    assert!(db.sql_as(&eve, "select code from public.invites").unwrap().is_empty(), "invites are their inviter's");
}

fn a_block_ends_hides_and_refuses(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" }));
    db.call(&bo, "POST", "/v1/friends", json!({ "name": "Anna" }));
    let (_, g) = db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess" }));
    assert_eq!(db.call(&bo, "POST", "/v1/blocks", json!({ "name": "Anna" })).0, 200);
    let st = friends(&db, &bo);
    assert_eq!(st["friends"], json!([]));
    assert_eq!(st["blocked"], json!(["Anna"]));
    assert_eq!(st["games"], json!([]), "her Games are hidden with her");
    let id = uuid::Uuid::parse_str(g["id"].as_str().unwrap()).unwrap();
    let status: String = db.owner().query_one("select status from public.games where id = $1", &[&id]).unwrap().get(0);
    assert_eq!(status, "cancelled", "her open challenge is cancelled");
    // Hidden from each other, and her requests and challenges refused.
    assert_eq!(db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" })).0, 404);
    assert_eq!(db.call(&anna, "POST", "/v1/games", json!({ "opponent": "Bo", "variant": "chess" })).0, 404);
    assert!(db.sql_as(&anna, &format!("select 1 from public.profiles where id = '{}'", bo.id)).unwrap().is_empty());
    let (a, b) = (anna.id.min(bo.id), anna.id.max(bo.id));
    assert!(db.sql_as(&anna, &format!("insert into public.friendships (a, b, asked_by) values ('{a}', '{b}', '{}')", anna.id)).is_err());
    let challenge = format!("insert into public.games (first, second, challenger, variant) values ('{0}', '{1}', '{0}', 'chess')", anna.id, bo.id);
    assert!(db.sql_as(&anna, &challenge).is_err());
    assert!(db.sql_as(&anna, "select * from public.blocks").unwrap().is_empty(), "she cannot read who blocks her");
    assert_eq!(db.call(&bo, "POST", "/v1/blocks/remove", json!({ "name": "Anna" })).0, 200);
    assert_eq!(db.call(&anna, "POST", "/v1/friends", json!({ "name": "Bo" })).0, 200, "unblocked");
}

fn a_report_reaches_the_operator_only(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    db.user("Bo");
    assert_eq!(db.call(&anna, "POST", "/v1/reports", json!({ "name": "Bo", "reason": "rude in Chat" })).0, 200);
    assert!(db.sql_as(&anna, "select * from public.reports").is_err(), "no player reads reports");
    let n: i64 = db.owner().query_one("select count(*) from public.reports where reason = 'rude in Chat'", &[]).unwrap().get(0);
    assert_eq!(n, 1);
    // A player you blocked can still be reported.
    db.call(&anna, "POST", "/v1/blocks", json!({ "name": "Bo" }));
    assert_eq!(db.call(&anna, "POST", "/v1/reports", json!({ "name": "Bo", "reason": "again" })).0, 200);
    assert_eq!(db.call(&anna, "POST", "/v1/reports", json!({ "name": "Nobody", "reason": "x" })).0, 404);
}

fn club_members_are_friends_while_both_allow_it(pg: &Pg) {
    let db = pg.fresh();
    let anna = db.user("Anna");
    let bo = db.user("Bo");
    let eve = db.user("Eve");
    let (_, club) = db.call(&anna, "POST", "/v1/clubs", json!({ "name": "Skakklubben" }));
    let code = club["code"].as_str().unwrap();
    assert_eq!(db.call(&bo, "POST", "/v1/clubs/join", json!({ "code": code })), (200, json!({ "name": "Skakklubben" })));
    assert_eq!(friends(&db, &bo)["clubs"][0]["members"], 2);
    assert_eq!(friends(&db, &bo)["clubs"][0]["code"], "", "only its maker hands out the code");
    assert!(db.sql_as(&eve, "select 1 from public.clubs").unwrap().is_empty());
    assert!(db.sql_as(&eve, "select 1 from public.club_members").unwrap().is_empty());
    assert_eq!(friends(&db, &anna)["friends"], json!([]), "not while neither allows it");
    db.call(&anna, "PUT", "/v1/me", json!({ "club_friends": true }));
    assert_eq!(friends(&db, &anna)["friends"], json!([]), "not while only one allows it");
    db.call(&bo, "PUT", "/v1/me", json!({ "club_friends": true }));
    assert_eq!(friends(&db, &anna)["friends"], json!([{ "name": "Bo", "club": true }]));
    assert_eq!(friends(&db, &anna)["club_friends"], true);
    db.call(&bo, "POST", "/v1/blocks", json!({ "name": "Anna" }));
    assert_eq!(friends(&db, &anna)["friends"], json!([]), "a block wins");
    let id = club["id"].as_str().unwrap();
    db.call(&bo, "POST", "/v1/blocks/remove", json!({ "name": "Anna" }));
    db.call(&bo, "POST", "/v1/clubs/leave", json!({ "id": id }));
    assert_eq!(friends(&db, &anna)["friends"], json!([]));
}

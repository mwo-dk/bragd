use super::*;
use crate::fake;

/// Waits for `what` to hold; the limit only guards against a hang.
fn until(ogs: &Ogs, what: impl Fn(&View) -> bool) -> View {
    for _ in 0..1000 {
        let v = ogs.view();
        if what(&v) {
            return v;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("never came: {:?}", ogs.view());
}

/// The browser: follows the sign-in page's redirect back to Bragd with a code.
fn browser(url: &str) -> Result<(), String> {
    let q = url.split_once('?').unwrap().1;
    let p = |k: &str| q.split('&').find_map(|x| x.strip_prefix(&format!("{k}="))).map(oauth::decode).unwrap();
    assert!(url.contains("/oauth2/authorize/?") && p("scope") == "read write" && p("code_challenge_method") == "S256", "{url}");
    let (redirect, state) = (p("redirect_uri"), p("state"));
    std::thread::spawn(move || {
        let addr = redirect.trim_start_matches("http://").trim_end_matches('/').to_string();
        let mut c = std::net::TcpStream::connect(&addr).unwrap();
        use std::io::{Read, Write};
        write!(c, "GET /?code=ogs_code&state={state} HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
        let _ = c.read_to_string(&mut String::new());
    });
    Ok(())
}

fn signed_in() -> (fake::Fake, Ogs) {
    bragd_model::key::mock();
    let f = fake::ogs();
    let ogs = Ogs::new(&f.url);
    ogs.sign_in(&browser).unwrap();
    (f, ogs)
}

/// The texts the socket was sent, as `[command, data]`.
fn said(f: &fake::Fake, command: &str) -> Vec<Value> {
    f.said().iter().filter_map(|t| serde_json::from_str::<Value>(t).ok()).filter(|v| v[0] == command).map(|v| v[1].clone()).collect()
}

#[test]
fn ogs_points_turn_both_ways() {
    // OGS counts from the top-left; Bragd names the row from the bottom and skips i.
    assert_eq!(entry(3, 3, 19).as_deref(), Some("d16"));
    assert_eq!(entry(3, 15, 19).as_deref(), Some("d4"));
    assert_eq!(entry(8, 0, 19).as_deref(), Some("j19"), "the ninth column is j");
    assert_eq!(entry(0, 8, 9).as_deref(), Some("a1"));
    assert_eq!(entry(-1, -1, 19).as_deref(), Some("pass"));
    assert_eq!(entry(19, 0, 19), None);
    assert_eq!(entry(9, 0, 9), None, "off a 9×9 board");
    for (e, size) in [("d16", 19), ("j19", 19), ("t1", 19), ("a1", 9), ("pass", 13)] {
        let m = ogs_move(e, size).unwrap();
        let b = m.as_bytes();
        let back = if m == ".." { entry(-1, -1, size) } else { entry((b[0] - b'a') as i64, (b[1] - b'a') as i64, size) };
        assert_eq!(back.as_deref(), Some(e), "{e} → {m}");
    }
    assert_eq!(ogs_move("d4", 19).as_deref(), Some("dp"));
    assert_eq!(ogs_move("k10", 9), None, "off a 9×9 board");
    assert_eq!(points("ddpp", 19), ["d16", "q4"]);
}

#[test]
fn ogs_time_systems_are_worded() {
    let byo = json!({"system": "byoyomi", "main_time": 600, "period_time": 30, "periods": 5});
    assert_eq!(time_label(&byo), t!("online.minutes_periods", "m" => 10, "n" => 5, "s" => 30));
    assert_eq!(time_label(&json!({"system": "fischer", "initial_time": 300, "time_increment": 10, "max_time": 600})), t!("online.minutes_plus", "m" => 5, "s" => 10));
    assert_eq!(time_label(&json!({"system": "canadian", "main_time": 600, "period_time": 300, "stones_per_period": 25})), t!("online.tc.canadian", "m" => 10, "n" => 25, "p" => 5));
    assert_eq!(time_label(&json!({"system": "simple", "per_move": 30})), t!("online.tc.simple", "s" => 30));
    assert_eq!(time_label(&json!({"system": "absolute", "total_time": 1800})), t!("online.tc.absolute", "m" => 30));
    assert_eq!(time_label(&json!({"system": "none"})), t!("online.tc.none"));
    assert_eq!((rank(20.0), rank(24.3), rank(30.0), rank(31.9)), ("10k".into(), "6k".into(), "1d".into(), "2d".into()));
}

#[test]
fn ogs_sign_in_keeps_the_token_and_opens_the_socket() {
    let (f, ogs) = signed_in();
    assert_eq!(bragd_model::key::get_in(SERVICE, &f.url).as_deref(), Some(fake::OGS_TOKEN));
    let v = until(&ogs, |v| v.connected && !v.challenges.is_empty());
    assert_eq!(v.signed_in.as_deref(), Some("BragdTester"));
    let token = f.seen().into_iter().find(|r| r.path == "/oauth2/token/").unwrap();
    assert!(token.body.contains("code=ogs_code") && token.body.contains("code_verifier=") && token.body.contains("client_id=bragd-fake"), "{}", token.body);
    let auth = said(&f, "authenticate");
    assert_eq!(auth[0]["jwt"], "jwt_fake", "the socket signs in with the account's key");
    let c = &v.challenges[0];
    assert!(c.incoming && c.rated && c.variant == Some("go-19"));
    assert_eq!((c.who.as_str(), c.time.clone()), ("Anna (6k)", t!("online.minutes_periods", "m" => 10, "n" => 5, "s" => 30)));
    // A second window's OGS finds the token kept.
    let other = Ogs::new(&f.url);
    other.open().unwrap();
    assert_eq!(other.view().signed_in.as_deref(), Some("BragdTester"));
    ogs.sign_out().unwrap();
    assert_eq!(bragd_model::key::get_in(SERVICE, &f.url), None);
    assert!(f.seen().iter().any(|r| r.path == "/oauth2/revoke_token/" && r.body.contains("token=ogs_fake")), "the token is taken back");
}

#[test]
fn ogs_without_a_registered_client_sends_nothing() {
    bragd_model::key::mock();
    let f = fake::ogs();
    let ogs = Ogs { client_id: "", ..Ogs::new(&f.url) };
    assert_eq!(ogs.sign_in(&|_| panic!("no browser")).unwrap_err(), t!("online.error.not_registered", "server" => "OGS"));
    assert!(f.seen().is_empty());
}

#[test]
fn ogs_ranked_game_locks_moves_are_checked_and_resign_ends_it() {
    let (f, ogs) = signed_in();
    until(&ogs, |v| !v.challenges.is_empty());
    ogs.answer("55", "accept").unwrap();
    let g = until(&ogs, |v| v.game.is_some()).game.unwrap();
    assert!(g.rated && g.against_person && !g.over);
    assert_eq!((g.seat, g.names[1].as_str(), g.byoyomi, g.periods, g.variant), (Some(0), "Anna (6k)", 30, [5, 5], Some("go-19")));
    assert_eq!(ogs.live(), Some((true, true, false)), "locked");
    assert!(ogs.view().challenges.is_empty() || ogs.view().challenges.iter().all(|c| c.id != "55"));
    let before = f.said().len();
    assert!(ogs.play("z9").is_err(), "refused by Bragd's rules");
    assert_eq!(f.said().len(), before, "nothing sent for an illegal move");
    ogs.play("d4").unwrap();
    let g = until(&ogs, |v| v.game.as_ref().is_some_and(|g| g.entries.len() == 2)).game.unwrap();
    assert_eq!(g.entries, ["d4", "q16"]);
    assert_eq!(said(&f, "game/move")[0]["move"], "dp");
    assert_eq!(g.state.unwrap().history.len(), 2);
    assert_eq!(ogs.draw(true).unwrap_err(), t!("online.error.no_draw", "server" => "OGS"));
    ogs.resign().unwrap();
    let g = until(&ogs, |v| v.game.as_ref().is_some_and(|g| g.over)).game.unwrap();
    assert_eq!(g.result, format!("{} · Resignation", t!("play.wins", "side" => t!("side.white"))));
    assert_eq!(ogs.live(), Some((true, true, true)), "over: unlocked");
    ogs.leave();
    assert!(ogs.view().game.is_none());
}

#[test]
fn ogs_seek_keeps_its_challenge_open_until_taken_and_the_score_is_agreed() {
    let (f, ogs) = signed_in();
    until(&ogs, |v| v.connected);
    assert_eq!(ogs.seek("go-19", "3+0", false).unwrap_err(), t!("online.error.time", "server" => "OGS"));
    ogs.seek("go-9", "10|5x30", false).unwrap();
    let body: Value = serde_json::from_str(&f.seen().into_iter().find(|r| r.path == "/api/v1/challenges/").unwrap().body).unwrap();
    assert_eq!((body["game"]["rules"].as_str(), body["game"]["width"].as_u64(), body["game"]["ranked"].as_bool()), (Some("chinese"), Some(9), Some(false)));
    assert_eq!(body["game"]["time_control_parameters"]["main_time"], 600);
    let v = until(&ogs, |v| v.game.is_some() && !v.seeking);
    assert!(!v.game.unwrap().rated);
    assert_eq!(said(&f, "challenge/keepalive")[0]["challenge_id"], 66);
    assert_eq!(ogs.live(), Some((false, true, false)), "casual: not locked");
    ogs.play("pass").unwrap();
    let g = until(&ogs, |v| v.game.as_ref().is_some_and(|g| g.dead.is_some())).game.unwrap();
    assert_eq!(g.dead.unwrap(), ["c16"]);
    ogs.accept_score().unwrap();
    let g = until(&ogs, |v| v.game.as_ref().is_some_and(|g| g.over)).game.unwrap();
    assert_eq!(said(&f, "game/removed_stones/accept")[0]["stones"], "cd");
    assert_eq!(g.result, format!("{} · 7.5 points", t!("play.wins", "side" => t!("side.black"))));
}

#[test]
fn ogs_challenge_finds_the_player_and_a_refusal_is_shown() {
    let (f, ogs) = signed_in();
    until(&ogs, |v| v.connected);
    assert_eq!(ogs.challenge("x/../y", "go-19", "10|5x30", true, "random").unwrap_err(), t!("online.error.user", "server" => "OGS"));
    assert_eq!(ogs.challenge("nobody", "go-19", "10|5x30", true, "random").unwrap_err(), t!("online.error.no_user", "server" => "OGS", "user" => "nobody"));
    ogs.challenge("anna", "go-13", "20|5x30", true, "white").unwrap();
    let r = f.seen().into_iter().find(|r| r.path == "/api/v1/players/2002/challenge").unwrap();
    let body: Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!((body["challenger_color"].as_str(), body["game"]["width"].as_u64()), (Some("white"), Some(13)));
    let v = until(&ogs, |v| v.error.is_some());
    assert_eq!(v.error, Some(t!("online.error.declined", "server" => "OGS", "why" => "Anna is busy")));
    assert!(v.challenges.iter().all(|c| c.incoming), "the refused challenge has gone");
}

#[test]
fn ogs_chat_is_off_until_turned_on() {
    let (f, ogs) = signed_in();
    until(&ogs, |v| !v.challenges.is_empty());
    ogs.answer("55", "accept").unwrap();
    until(&ogs, |v| v.game.is_some());
    assert_eq!(ogs.say("hi").unwrap_err(), t!("online.error.chat_off"));
    assert!(said(&f, "game/connect").iter().all(|c| c["chat"] == false));
    ogs.set_chat(true).unwrap();
    until(&ogs, |v| v.chat.len() == 1);
    ogs.say("well played").unwrap();
    let v = until(&ogs, |v| v.chat.len() == 3);
    assert_eq!(v.chat[1], ChatLine { user: "BragdTester".into(), text: "well played".into() });
    ogs.set_chat(false).unwrap();
    assert!(ogs.view().chat.is_empty());
}

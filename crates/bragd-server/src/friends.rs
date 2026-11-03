//! Friends, blocks, reports and Clubs. A Friend is another player you both accepted, by a
//! request or an invite code; members of a Club you share count too while both allow it. A
//! block ends the friendship, cancels open challenges between the two, and hides each from the
//! other. A report goes to the operator's table, which no player can read.

use postgres::Transaction;
use serde_json::{Value, json};

use crate::api::{Error, Res, Service, text};
use crate::db;
use crate::jwt::Caller;

/// The id of the player named `name`, as the caller may see them: not themselves, and not one
/// either of them blocks.
pub fn player(tx: &mut Transaction, name: &str) -> Result<uuid::Uuid, Error> {
    Ok(tx.query_opt("select id from public.profiles where name = $1 and id <> auth.uid()", &[&name])?.ok_or(Error(404, "no_user"))?.get(0))
}

/// A code to hand to someone: 16 random bytes in base64url.
fn code() -> String {
    use base64::Engine;
    let mut b = [0u8; 16];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut b).expect("the OS random source");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// The caller's Friends and requests, blocks, Clubs and Club setting, for `GET /v1/state`.
pub fn state(tx: &mut Transaction) -> Result<Value, Error> {
    let names = |rows: Vec<postgres::Row>| rows.iter().map(|r| r.get::<_, String>(0)).collect::<Vec<_>>();
    let friends: Vec<Value> = tx
        .query(
            "select p.name, bool_and(x.club) from ( \
               select case when f.a = auth.uid() then f.b else f.a end as id, false as club from public.friendships f where f.accepted \
               union all select o.member, true from public.club_members m join public.club_members o on o.club_id = m.club_id and o.member <> m.member \
               where m.member = auth.uid()) x \
             join public.profiles p on p.id = x.id where bragd.are_friends(auth.uid(), x.id) group by p.name order by p.name",
            &[],
        )?
        .iter()
        .map(|r| json!({ "name": r.get::<_, String>(0), "club": r.get::<_, bool>(1) }))
        .collect();
    let pending = |tx: &mut Transaction, mine: bool| -> Result<Vec<String>, Error> {
        let sql = format!(
            "select p.name from public.friendships f join public.profiles p on p.id = case when f.a = auth.uid() then f.b else f.a end \
             where not f.accepted and (f.asked_by = auth.uid()) = {mine} order by f.created_at"
        );
        Ok(names(tx.query(&sql, &[])?))
    };
    let asked = pending(tx, false)?;
    let asking = pending(tx, true)?;
    // A blocked player's profile is hidden from the blocker too, so their name is read as the
    // referee, for the caller's own blocks only.
    let me: uuid::Uuid = tx.query_one("select auth.uid()", &[])?.get(0);
    db::referee(tx)?;
    let blocked = names(tx.query("select p.name from public.blocks b join public.profiles p on p.id = b.blocked where b.blocker = $1 order by p.name", &[&me])?);
    tx.batch_execute("set local role authenticated")?;
    let clubs: Vec<Value> = tx
        .query(
            "select c.id, c.name, c.code, c.created_by = auth.uid(), (select count(*) from public.club_members m where m.club_id = c.id) \
             from public.clubs c order by c.name",
            &[],
        )?
        .iter()
        .map(|r| {
            let own: bool = r.get(3);
            json!({ "id": r.get::<_, uuid::Uuid>(0).to_string(), "name": r.get::<_, String>(1), "code": if own { r.get::<_, String>(2) } else { String::new() }, "members": r.get::<_, i64>(4) })
        })
        .collect();
    let club_friends = tx.query_opt("select club_friends from public.profiles where id = auth.uid()", &[])?.is_some_and(|r| r.get(0));
    Ok(json!({ "friends": friends, "asked": asked, "asking": asking, "blocked": blocked, "clubs": clubs, "club_friends": club_friends }))
}

impl Service {
    pub(crate) fn friends(&self, caller: &Caller, what: &[&str], body: &Value) -> Res {
        self.pool.as_user(caller, |tx| match what {
            // Asks `name` to be Friends, or accepts when they asked first.
            ["friends"] => {
                let them = player(tx, text(body, "name")?)?;
                let (a, b) = (caller.id.min(them), caller.id.max(them));
                let row = tx.query_opt("select asked_by, accepted from public.friendships where a = $1 and b = $2", &[&a, &b])?;
                match row.map(|r| (r.get::<_, uuid::Uuid>(0), r.get::<_, bool>(1))) {
                    None => {
                        tx.execute("insert into public.friendships (a, b, asked_by) values ($1, $2, auth.uid())", &[&a, &b])?;
                        Ok(json!({ "friends": false }))
                    }
                    Some((by, false)) if by == them => {
                        tx.execute("update public.friendships set accepted = true where a = $1 and b = $2", &[&a, &b])?;
                        Ok(json!({ "friends": true }))
                    }
                    Some((_, accepted)) => Ok(json!({ "friends": accepted })),
                }
            }
            // Declines a request, takes one back, or ends a friendship.
            ["friends", "remove"] => {
                let them = player(tx, text(body, "name")?)?;
                tx.execute("delete from public.friendships where a = $1 and b = $2", &[&caller.id.min(them), &caller.id.max(them)])?;
                Ok(json!({}))
            }
            ["invites"] => {
                let code = code();
                tx.execute("insert into public.invites (code, inviter) values ($1, auth.uid())", &[&code])?;
                Ok(json!({ "code": code }))
            }
            // The invite's code makes its inviter and the caller Friends. Used once.
            ["invites", "redeem"] => {
                let code = text(body, "code")?;
                db::referee(tx)?;
                let inviter: uuid::Uuid = tx.query_opt("delete from public.invites where code = $1 returning inviter", &[&code])?.ok_or(Error(404, "no_invite"))?.get(0);
                if inviter == caller.id {
                    return Err(Error(409, "own_invite"));
                }
                let blocked: bool = tx.query_one("select bragd.blocked_between($1, $2)", &[&inviter, &caller.id])?.get(0);
                if blocked {
                    return Err(Error(404, "no_invite"));
                }
                tx.execute(
                    "insert into public.friendships (a, b, asked_by, accepted) values ($1, $2, $3, true) on conflict (a, b) do update set accepted = true",
                    &[&caller.id.min(inviter), &caller.id.max(inviter), &inviter],
                )?;
                let name: String = tx.query_one("select name from public.profiles where id = $1", &[&inviter])?.get(0);
                Ok(json!({ "name": name }))
            }
            ["blocks"] => {
                let them = player(tx, text(body, "name")?)?;
                tx.execute("insert into public.blocks (blocker, blocked) values (auth.uid(), $1) on conflict do nothing", &[&them])?;
                db::referee(tx)?;
                tx.execute("delete from public.friendships where a = $1 and b = $2", &[&caller.id.min(them), &caller.id.max(them)])?;
                tx.execute(
                    "update public.games set status = 'cancelled', updated_at = now() where status = 'offered' and ((first = $1 and second = $2) or (first = $2 and second = $1))",
                    &[&caller.id, &them],
                )?;
                Ok(json!({}))
            }
            ["blocks", "remove"] => {
                let name = text(body, "name")?;
                // The blocked player's profile is hidden from the caller: found as the referee,
                // among the caller's own blocks only.
                db::referee(tx)?;
                tx.execute("delete from public.blocks b using public.profiles p where p.id = b.blocked and b.blocker = $1 and p.name = $2", &[&caller.id, &name])?;
                Ok(json!({}))
            }
            ["reports"] => {
                let reason = text(body, "reason")?;
                let them = match player(tx, text(body, "name")?) {
                    Ok(id) => id,
                    // A player the caller blocked is still reported.
                    Err(_) => {
                        let name = text(body, "name")?;
                        let me = caller.id;
                        db::referee(tx)?;
                        let row = tx.query_opt("select b.blocked from public.blocks b join public.profiles p on p.id = b.blocked where b.blocker = $1 and p.name = $2", &[&me, &name])?;
                        tx.batch_execute("set local role authenticated")?;
                        row.ok_or(Error(404, "no_user"))?.get(0)
                    }
                };
                let game = body["game"].as_str().and_then(|g| uuid::Uuid::parse_str(g).ok());
                tx.execute("insert into public.reports (reported, game_id, reason) values ($1, $2, $3)", &[&them, &game, &reason])?;
                Ok(json!({}))
            }
            ["clubs"] => {
                let name = text(body, "name")?;
                let code = code();
                // Its id is made here: the new Club is not the caller's to read back until they
                // are its member.
                let id = uuid::Uuid::new_v4();
                tx.execute("insert into public.clubs (id, name, code, created_by) values ($1, $2, $3, auth.uid())", &[&id, &name, &code])?;
                db::referee(tx)?;
                tx.execute("insert into public.club_members (club_id, member) values ($1, $2)", &[&id, &caller.id])?;
                Ok(json!({ "id": id.to_string(), "code": code }))
            }
            ["clubs", "join"] => {
                let code = text(body, "code")?;
                db::referee(tx)?;
                let row = tx.query_opt("select id, name from public.clubs where code = $1", &[&code])?.ok_or(Error(404, "no_club"))?;
                let id: uuid::Uuid = row.get(0);
                tx.execute("insert into public.club_members (club_id, member) values ($1, $2) on conflict do nothing", &[&id, &caller.id])?;
                Ok(json!({ "name": row.get::<_, String>(1) }))
            }
            ["clubs", "leave"] => {
                let id = uuid::Uuid::parse_str(text(body, "id")?).map_err(|_| Error(404, "no_club"))?;
                tx.execute("delete from public.club_members where club_id = $1 and member = auth.uid()", &[&id])?;
                Ok(json!({}))
            }
            _ => Err(Error(404, "not_found")),
        })
    }
}

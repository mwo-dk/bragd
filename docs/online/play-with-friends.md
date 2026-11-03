[← README](../../README.md) · [Docs index](../README.md) · [Online](README.md)

# Play with Friends

Play other Bragd players on the Bragd server. You make an account there, become Friends with the players you know, put your Club on the server, and play Games that the server's referee checks by Bragd's own rules. In F1 this is the `friends` entry.

![Play with Friends, 1100 px wide: the Friends list with Anna and Bo, a request from Carl, the Club Skakklubben, and Anna's challenge to chess](../screenshots/friends-1100.png)
*Play with Friends, signed in.*

## How to reach it

*Play with Friends* is in the sidebar, below Clubs, once a Bragd server address is set in **Settings → Online → Bragd server address**. Until then the page is hidden and Bragd sends nothing to any Bragd server. The address is empty until the Bragd server is up; this page will say when it is.

It also shows only while one of the Families the Bragd server plays is on: chess, xiangqi, shogi, go, draughts or tafl.

![Play with Friends before signing in, 390 px wide: the email and password fields, Sign in, Make an account and Sign in with Github](../screenshots/friends-sign-in-390.png)
*Signing in, 390 px wide.*

| Button | What it does |
|---|---|
| **Sign in** | Signs in with your email and password |
| **Make an account** | Makes an account with that email and password. The Bragd server may first send you an email with a link to confirm it |
| **Sign in with …** | Signs in with a provider the Bragd server offers, such as GitHub, on its page in your browser. Bragd waits up to five minutes |
| **Sign out** | Ends the session on the Bragd server and removes it from your keychain |

After the first sign-in you choose a name. Other players find you by it, and it is all they see of your account.

## Friends

A Friend is another player you both accepted.

| Button | What it does |
|---|---|
| **Ask** | Asks the player with that name to be your Friend. If they asked you first, you are Friends at once |
| **Accept** / **Decline** | Answers a player who asks you |
| **Take back** | Withdraws your own request |
| **Make an invite code** | Shows a code to give someone outside Bragd. Whoever enters it becomes your Friend. It works once |
| **Use code** | Enters an invite code you were given |
| **Remove** | Ends a friendship |
| **Block** | Ends the friendship, cancels open challenges between you, and hides you from each other. Their requests and challenges are refused. **Unblock** lifts it |
| **Report** | Sends what happened to the Bragd server's operator. The player is not told, and no player can read a report |

### Clubs

A Club on the Bragd server has a name and a code. Whoever puts it there sees the code and hands it to the members, who join with it.

With **Count the members of my Clubs as Friends** on, a member of one of your Clubs counts as a Friend, as long as they have it on too. A block still wins.

| Button | What it does |
|---|---|
| **Put a Club on the Bragd server** | Makes a Club with the name typed, and shows its code |
| **Join** | Joins the Club whose code you typed |
| **Leave** | Leaves the Club |

The Club on the Bragd server is not your Club file: its members, roles and ratings stay on your machine ([Clubs](../clubs/clubs.md)).

## Games

![A Game against Anna on the Bragd server, 1100 px wide: the board after 1. e4 a6, Your move, Offer a draw and Resign](../screenshots/friends-game-1100.png)
*A Game on the Bragd server.*

Challenge a player by their name, with the Variant and the Side you want, or answer a challenge. Anyone with a name can be challenged, not only Friends. The Game is played on Bragd's board as on [Lichess](playing-online.md), and the Bragd server's referee checks every move by Bragd's rules: a move that is illegal, out of turn or made after the Game moved on is refused.

| Button | What it does |
|---|---|
| **Challenge** | Challenges the player named, in the Variant chosen |
| **Accept** / **Decline** | Answers a challenge made to you |
| **Cancel** | Takes back a challenge you made |
| **Offer a draw** | Offers a draw; the other player accepts or declines |
| **Resign** | Gives up the Game |
| **Back to the lobby** | Leaves a Game that is over |

The Bragd server has no clocks, ratings or seeks yet, and does not play backgammon, Ur or senet: their dice would have to be thrown by the server.

## What it needs

- The Bragd server's address in Settings, and a connection to it over HTTPS.
- An account on the Bragd server, and your system's keychain for the sign-in, under the service `bragd-online` with the server's address as the account.
- For a provider's sign-in, a browser.

The Bragd server is Supabase, hosted in the EU, plus Bragd's own game service ([Privacy](../reference/privacy.md)). For testing, `BRAGD_FRIENDS_URL` names another address and `BRAGD_FRIENDS_TOKEN` stands in for the keychain, for a server on this machine only (`cargo run -p bragd-online --features fake --example fake-online -- bragd`).

## Questions

#### Does Bragd see my password?

Your email and password go to the Bragd server's sign-in, Supabase Auth, and nowhere else. Bragd keeps only the sign-in it is given, in your system's keychain.

#### What can other players see of me?

Your name. Your Friends see that you are Friends; the two players of a Game see its moves. Nobody sees whom you blocked or reported.

#### Why is Play with Friends not in my sidebar?

No Bragd server address is set yet, or none of the Families it plays is on.

#### Can a stranger challenge me?

Yes, by your name. Block them to refuse their challenges.

---
[← Previous: Playing on OGS](ogs.md) · [Next: pychess.org →](pychess.md)

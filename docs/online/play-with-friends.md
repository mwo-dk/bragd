[← README](../../README.md) · [Docs index](../README.md) · [Online](README.md)

# Play with Friends

Play other Bragd players on the Bragd server. You make an account there, become Friends with the players you know, put your Club on the server, and play Games that the server's referee checks by Bragd's own rules. In F1 this is the `friends` entry.

![Play with Friends, 1100 px wide: the Friends list with Anna and Bo, a request from Carl, the Club Skakklubben, and Anna's challenge to chess](../screenshots/friends-1100.png)
*Play with Friends, signed in.*

## Contents

- [How to reach it](#how-to-reach-it)
- [Friends](#friends)
- [Games](#games): [Clocks and ratings](#clocks-and-ratings), [Dice](#dice), [Chat](#chat), [Calls](#calls)
- [Deleting your account](#deleting-your-account)
- [What it needs](#what-it-needs)
- [Questions](#questions)

## How to reach it

*Play with Friends* is in the sidebar, below Clubs, once a Bragd server address is set in **Settings → Online → Bragd server address**. Until then the page is hidden and Bragd sends nothing to any Bragd server. F1, What's new, the tour cards and Tips leave out *Play with Friends*, *Chat* and *Calls* too, and Settings → Online shows only the address, not the Chat, filter, voice and camera choices. The address is empty until the Bragd server is up; this page will say when it is.

It also shows only while one of the Families the Bragd server plays is on. It plays every Family: chess, xiangqi, shogi, go, draughts, tafl, backgammon, Ur and senet.

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

Seek a Game that any player may take, challenge a player by their name, with the Variant and the Side you want, or answer a challenge. Anyone with a name can be challenged, not only Friends. The Game is played on Bragd's board as on [Lichess](playing-online.md), and the Bragd server's referee checks every move by Bragd's rules: a move that is illegal, out of turn or made after the Game moved on is refused.

| Button | What it does |
|---|---|
| **Seek a Game** | Offers a Game in the Variant, time and rating chosen to any player. Other players see it among their challenges. You have one seek at a time; it lasts an hour unless it is taken. **Cancel** takes it back |
| **Challenge** | Challenges the player named, in the Variant, time and rating chosen |
| **Accept** / **Decline** | Answers a challenge made to you, or takes another player's seek. **Decline** on a seek only hides it from you |
| **Cancel** | Takes back a challenge you made |
| **Offer a draw** | Offers a draw; the other player accepts or declines |
| **Resign** | Gives up the Game |
| **Back to the lobby** | Leaves a Game that is over |

### Clocks and ratings

![The lobby of Play with Friends, 1100 px wide: Seek a Game with 10 min + 5 s and Rated, your ratings, and Bo's seek for shogi among the challenges](../screenshots/friends-seek-1100.png)
*Seeking a Game, with Bo's seek beside it.*

| Control | What it does |
|---|---|
| **Time** | *No clock*, or 10 min + 5 s, 1 min + 0 s, 3 min + 2 s, 5 min + 3 s, 15 min + 10 s or 30 min + 20 s: each player's time, and what is added after each of their turns |
| **Rated** | The Game changes both players' ratings when it ends |

The clocks are kept by the Bragd server. It counts the time you use from the moment it recorded the other player's move to the moment it records yours, so the time your move takes to reach it counts against you. The clock starts when the Game starts. When your time runs out, the server ends the Game as lost on time; Bragd asks it to look when a clock shows nought, and a move sent after that is not played.

Each move reaches the other player at once: the server tells both players' Bragd over Supabase Realtime that the Game changed, and Bragd asks it how the Game stands. The message names only the Game, and only you can listen for yours. When Realtime cannot be reached, Bragd asks the server every two seconds instead, which is slow for a 1-minute Game.

A rating is Glicko-2, as [Clubs](../clubs/clubs.md) use: one for each Game family, starting at 1500, and changed after every rated Game ([ADR 0007](../adr/0007-ratings-on-the-bragd-server.md)). *Your ratings* above the seek shows yours in the Families you have on. A rating with a `?`, such as *1500?*, is still provisional: Glicko-2 is not yet sure of it. A Game ended by deleting an account is not rated.

During a rated Game, Analyse, Engines, book moves and tablebases are locked in every Bragd window, as on Lichess.

### Dice

In backgammon, Ur and senet the Bragd server throws the dice and sticks for both players, from its own random source. Neither player's Bragd throws, so neither can choose or foresee a throw. Each throw is kept with the Game's moves, and the move list shows it.

![A backgammon Game against Anna on the Bragd server, 1100 px wide: your turn to throw, with Throw and Double beside the board](../screenshots/friends-dice-1100.png)
*Your throw in backgammon: Throw, or Double first.*

The server throws as soon as a throw is due, so you only ever move. In backgammon, while you may double, it waits for you:

| Button | What it does |
|---|---|
| **Throw** | Asks the server to throw your dice |
| **Double** | Offers the other player a double before you throw. They **Take** or **Drop** it |

Backgammon's opening throw is one die each, thrown by the server too; the higher die moves first.

In backgammon you move one checker at a time, as in [local Play](../play/backgammon.md): drag a checker, or click it and then where it goes. Your checkers stay on your machine until the dice are used, then Bragd sends the whole turn and the server checks it. **Undo** takes back the last checker before then.

![A backgammon Game against Anna on the Bragd server, 1100 px wide: after the server's throw, one checker of your turn moved, with Undo beside the board](../screenshots/friends-step-1100.png)
*One checker moved; the turn goes to the server once the dice are used.*

### Chat

A Game's Chat is off. It opens only when both players say yes, and either closes it for both. In F1 this is the `chat` entry.

![A Game's Chat on the Bragd server, 1100 px wide: Anna's line and yours, Keep this Chat with the Game, and Close Chat for both](../screenshots/friends-chat-1100.png)
*Chat in a Game.*

| Button | What it does |
|---|---|
| **Ask to Chat** | Asks the other player. They see "… would like to Chat" with **Allow** and **Not now** |
| **Allow** / **Not now** | Answers the other player's request. **Not now** keeps it closed |
| **Send** | Says a line, up to 500 characters |
| **Keep this Chat with the Game** | The Chat is kept with the Game only when both players tick it. Otherwise its lines are deleted when the Game ends |
| **Close Chat for both** | Closes it. Opening it again takes both players' yes again |

You can Chat with a player who is not your Friend. Their lines then pass a word filter: a word on its lists shows as asterisks. The filter runs on your machine, and **Settings → Online → Filter a stranger's Chat** turns it off. A Friend's lines are never filtered.

The filter has a list for each of these languages, and every list applies to every line, whatever language you use Bragd in: Chinese, Czech, Danish, Dutch, English, Finnish, French, German, Italian, Japanese, Korean, Norwegian, Persian, Polish, Spanish and Swedish. It looks for whole words, so a word that only contains a listed one, such as *Scunthorpe*, stays. In Chinese and Japanese, which have no spaces between words, and in Korean, a listed word is found inside a longer one. Before it compares, it ignores capitals and accents, reads full-width letters as plain ones, and reads the usual stand-ins as letters: `sh1t` and `$hit` are masked as *shit* is. A word that is rude in one language but ordinary in another, such as *bitte* or *negro*, is left off the lists.

**Settings → Online → When a Bragd player asks to Chat**: *Ask me* (the default) shows a request; *Off* refuses it without showing it.

### Calls

During a Game with a Friend you can talk, and see each other. In F1 this is the `calls` entry. A Call is only ever between Friends: with a stranger the Call's buttons do not show.

![A Call with Anna beside the board, 1100 px wide: Ask for voice pressed and waiting for Anna, Ask for camera, and the line that nothing is recorded](../screenshots/friends-call-1100.png)
*Asking a Friend for voice.*

Voice and camera are separate channels. Each opens only when both of you say yes, and either of you closes it for both.

| Button | What it does |
|---|---|
| **Ask for voice** / **Ask for camera** | Asks the other player. They see "… would like voice" with **Allow** and **Not now** |
| **Allow** / **Not now** | Answers the other player's request |
| **Stop voice for both** / **Stop camera for both** | Closes that channel. Opening it again takes both players' yes again |

While your microphone or camera is live, a red line beside the board says *Your microphone is on* or *Your camera is on*. The Call ends with the Game, and when either of you stops being the other's Friend.

**Settings → Online → When a Friend asks for voice** and **… for the camera**: *Ask me* (the default) shows a request; *Off* refuses it without showing it.

The sound and picture go directly between your two machines, encrypted end to end (WebRTC). When no direct path exists, the Bragd server's TURN relay passes the encrypted packets on without being able to read them. The Bragd server only brings the two machines together. Nothing of a Call is recorded, by Bragd or by the server.

## Deleting your account

**Delete my account**, below the Games (or below *Choose the name other players find you by*, if you have not chosen one), deletes your account on the Bragd server and everything with it, at once: your name, Friends, requests, invite codes, blocks, reports, the Clubs you made, your Chat lines and your Games ([Privacy](../reference/privacy.md#the-bragd-server) lists it all). It cannot be undone.

![Delete my account, 1100 px wide: what is deleted, Leave my finished Games to the other players, the name typed to confirm, and Delete my account for good](../screenshots/friends-delete-1100.png)
*Deleting the account.*

| Control | What it does |
|---|---|
| **Delete my account** | Opens the form. Nothing is deleted yet |
| **Leave my finished Games to the other players, as a deleted player** | Off: your Games go too. On: Games that ended stay with the other player, who sees *Deleted player* as their opponent. A Game under way ends as your resignation |
| **Type your name** | Your name on the Bragd server, as it is. An account without a name types the word the form shows instead, such as *delete*. The button works only when it matches |
| **Delete my account for good** | Deletes the account, signs out, and removes the sign-in from your keychain |

You can make a new account afterwards with the same email; it starts empty.

## What it needs

- The Bragd server's address in Settings, and a connection to it over HTTPS, and to its Supabase Realtime over a WebSocket for moves at once.
- An account on the Bragd server, and your system's keychain for the sign-in, under the service `bragd-online` with the server's address as the account.
- For a provider's sign-in, a browser.
- For Calls on Linux, GStreamer's WebRTC plugins (`webrtcbin` from plugins-bad, and libnice's `nice`). The Flatpak's runtime has them, the `.deb` and `.rpm` depend on them, and the AppImage carries them. Built from source, install `gstreamer1.0-plugins-bad` and `gstreamer1.0-nice` (Debian, Ubuntu) or `gstreamer1-plugins-bad-free` and `libnice-gstreamer1` (Fedora). On Flatpak, Bragd may use the camera and PulseAudio for Calls.

The Bragd server is Supabase, hosted in the EU, plus Bragd's own game service ([Privacy](../reference/privacy.md)). For testing, `BRAGD_FRIENDS_URL` names another address and `BRAGD_FRIENDS_TOKEN` stands in for the keychain, for a server on this machine only (`cargo run -p bragd-online --features fake --example fake-online -- bragd`).

## Questions

#### Does Bragd see my password?

Your email and password go to the Bragd server's sign-in, Supabase Auth, and nowhere else. Bragd keeps only the sign-in it is given, in your system's keychain.

#### What can other players see of me?

Your name. Your Friends see that you are Friends; the two players of a Game see its moves. Nobody sees whom you blocked or reported.

#### Why is Play with Friends not in my sidebar?

No Bragd server address is set yet, or none of the Families it plays is on. Without an address, F1 and What's new do not offer it either.

#### Can I add a word to the Chat filter?

Not in Bragd itself. The lists are part of Bragd, one file a language in `crates/bragd-online/src/words/`, so a word is added by a pull request or an issue on GitHub. A language without a list yet gets one the same way.

#### Who can read a Chat?

The two players of the Game, and nobody else. Its lines are on the Bragd server while the Game is on, and deleted when it ends unless both of you ticked **Keep**.

#### Why can I not call this player?

A Call is only between Friends. Ask them to be your Friend, or share a Club with Club Friends turned on.

#### Why does my operating system ask about the camera?

Windows and macOS ask once whether Bragd may use the camera and the microphone. Bragd opens them only while a channel is open for both of you; on Linux it refuses the webview any other time.

#### How do I delete my account, and how long does it take?

Press **Delete my account** below the Games, type your name (or, without a name, the word the form shows) and press **Delete my account for good**. It is deleted on the Bragd server at once. The server's backups hold it for up to 30 days more ([Privacy](../reference/privacy.md#the-bragd-server)).

#### Is a 1-minute Game fair on the Bragd server?

Yes, while Supabase Realtime can be reached: the server tells your Bragd of each move at once, and the clocks are the server's. If Realtime is down, Bragd asks the server every two seconds, and in a 1-minute Game that delay costs you time. Choose a slower clock then.

#### How is my rating worked out?

By Glicko-2, after each rated Game, from your rating, your opponent's and the result. Its deviation says how sure it is; while it is high the rating moves fast and shows a `?`. Each Game family has its own rating, so chess and go do not mix.

#### Who throws the dice in backgammon, Ur and senet?

The Bragd server, for both players, from its own random source. Neither player's Bragd can choose a throw, and the server refuses a throw sent as a move. Each throw is kept with the Game's moves.

#### Can a stranger challenge me?

Yes, by your name. Block them to refuse their challenges.

---
[← Previous: Playing on OGS](ogs.md) · [Next: pychess.org →](pychess.md)

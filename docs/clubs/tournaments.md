[← README](../../README.md) · [Docs index](../README.md) · [Clubs](README.md)

# Tournaments

A Club runs its own Tournaments: a Swiss paired by FIDE's Dutch rules, or a single or double round-robin by FIDE's Berger tables. On the night you check players in, pair the Round, show the pairings on a second screen, enter the results and publish the standings. A director can change any pairing. Bragd says which rules the change breaks and keeps it in the Tournament's log with the reason. Players level on points are ordered by FIDE's tiebreaks, in the order you choose. A Tournament is kept in its Club's file, so a copy of the Club carries its Tournaments. In F1 → Features this is the entry `tournaments`.

![The Rounds tab of a Tournament: Round 1 with its boards and results, the buttons Show on a second screen, Change the pairing and Pair again](../screenshots/tournaments-1100.png)
*A Tournament open on its Rounds tab.*

![The Standings tab at 390 px: places, points and the tiebreak columns, then the list of tiebreaks](../screenshots/tournaments-390.png)
*The standings at 390 px.*

## Contents

- [How to reach it](#how-to-reach-it)
- [On the night](#on-the-night)
- [Changing a pairing](#changing-a-pairing)
- [Results and corrections](#results-and-corrections)
- [Tiebreaks](#tiebreaks)
- [Publishing and exports](#publishing-and-exports)
- [Club ratings](#club-ratings)
- [What it needs](#what-it-needs)
- [Settings and config.toml](#settings-and-configtoml)
- [Questions](#questions)

## How to reach it

**Clubs** in the sidebar, open a Club, then **Tournaments** below its members. Choose who you are under **Acting as**: a director or an arbiter runs a Tournament, and only a director deletes one, changes a pairing by hand or rates the members' games.

| Action | What it does |
|---|---|
| **Create Tournament** | Sets up a Tournament from its name, pairing system, number of Rounds (Swiss), first day and place |
| **Open a bundle…** | Adds the Tournament in a `.bragd-tournament` bundle to the open Club |
| **Delete Tournament** | Deletes it with its Rounds and results, after asking |

A Tournament has five tabs: **Players**, **Rounds**, **Standings**, **Publish and export** and **Log**.

## On the night

| Step | Where | What you do |
|---|---|---|
| Register | Players | Pick a **Club member**, or find a **Guest from a rating list** by name or ID, or type a guest in. **Check them in at once** checks a player in as they are added |
| Check in | Players | Set each player's **Status**: *Checked in*, *Not here*, *Half-point bye next Round* or *Withdrawn*. **Check in everyone** checks in all who are *Not here* |
| Pair | Rounds | **Pair Round N**. It is greyed out, with the reason beside it, until every result of the last Round is in |
| Show | Rounds | **Show on a second screen** opens the pairings in large type in a window of their own. Drag it to a projector or print it from there |
| Enter results | Rounds | Choose each board's result: 1-0, ½-½, 0-1, a forfeit either way, or neither played |
| Publish | Publish and export | Save **Standings and crosstable (HTML)** and put it where the club can see it |

Only players who are checked in are paired. A player with a half-point bye gets half a point for the next Round and is checked in again after it. A player who is not here gets nothing for the Round. A player who withdraws keeps their results and is never paired again. A late entry starts with no points for the Rounds before; in a Swiss they are paired from the next Round. If someone arrives after the pairing, check them in and press **Pair again**, giving the reason.

A Swiss gives the player left over a bye worth a point. A round-robin fixes everyone's number when Round 1 is paired, by rating, and takes no new players after that. With an odd number of players, one sits out each Round with no points.

## Changing a pairing

**Change the pairing** lets a director choose White and Black on each board, and the bye, while no result of the Round is in. **Check against the rules** then says what the change breaks:

| Warning | Meaning |
|---|---|
| Breaks an absolute rule | Two players meet again, someone gets a second bye, or the rest of the Round cannot be paired |
| The rules' pairing does better on … | Legal, but worse on one of the Dutch system's criteria, C5 to C21, named in words |
| The rules take another one first | As good on every criterion, but not the one the rules choose |
| Colours differ from the rules | The same pairs, with colours the other way on the boards named |
| Not the Berger table's pairing | In a round-robin, the boards differ from the table |
| … is checked in but not paired | Someone who is here has no board |

You can keep a pairing with warnings. **Keep this pairing** needs a reason, and the log keeps the old pairing, the new one, the warnings and the reason.

## Results and corrections

A result entered for the first time is not logged. Changing a result that is already there asks why, and the log keeps the old and new result with the reason. Earlier Rounds are under the current one, folded; open one to correct it.

## Tiebreaks

The **Standings** tab ranks players by points, then by the first tiebreak, then the next, as FIDE's Tie-Break Regulations (C.07, from 1 March 2026) say. Players level on everything share a place. **Change the tiebreaks** reorders, adds and removes them.

| Code | Tiebreak |
|---|---|
| DE | Direct encounter |
| WIN | Wins, with or without playing |
| WON | Games won over the board |
| BPG | Games with Black |
| BWG | Wins with Black |
| PS | Progressive score |
| BH | Buchholz |
| BH-C1, BH-C2 | Buchholz without the lowest one or two opponents |
| BH-M1, BH-M2 | Buchholz without the lowest and highest one or two |
| AOB | Average of opponents' Buchholz |
| SB, SB-C1 | Sonneborn-Berger, and without the lowest |
| KS | Koya |
| ARO, ARO-C1 | Average rating of opponents, and without the lowest |
| TPR | Performance rating |
| RTNG | The player's own rating |

A new Tournament starts with the Danish Chess Union's defaults (EMT rules, §24): a Swiss uses *almindelig korrektion*, which is BH-C2, then BH-C1, then BH; a round-robin uses SB.

In a Swiss, rounds a player did not play count as C.07 Article 16 says. For an opponent's Buchholz, a player's byes or absences that are followed only by more of them count as draws. For a player's own Buchholz, each unplayed Round counts as a game against an opponent with the player's own score, but at most a draw a Round, or the scheduled opponent's score for a forfeit. Cutting the lowest value cuts a Round the player chose not to play first. In a round-robin, forfeits count as games. The rating tiebreaks leave unrated opponents out.

## Publishing and exports

| Export | What it holds |
|---|---|
| **Standings and crosstable (HTML)** | One page with the standings and every tiebreak, the crosstable and every Round's pairings and results. It needs no Bragd to read |
| **FIDE report (TRF)** | FIDE's Tournament Report File (TRF-2026), with the tiebreaks in its `202` line. Pairing programs and FIDE rating reports read it |
| **Games (PGN)** | Every game played over the board, with the players, ratings, FIDE IDs, Round, board and result, but no moves |
| **Bragd Tournament bundle** | The whole Tournament with its log, plus the three files above, in one `.bragd-tournament` file. **Open a bundle…** reads it into a Club |

The Danish Chess Union publishes no file format for rating reports. Results are typed into its web system, turnering.skak.dk, which takes the games as PGN, so there is no DSU export beyond the PGN.

## Club ratings

When every result is in, **Rate the members' games** works out the club ratings of the members from the games they played each other, as one rating period. Guests and forfeits do not count. It can be done once per Tournament, and the Club's audit log names the Tournament.

## What it needs

Nothing beyond an open Club. Guests from a rating list need the list, which needs the [Helper](../knowledge/helper.md) and a download you agree to.

## Settings and config.toml

Tournaments have no settings. Each Tournament's system, Rounds and tiebreaks are its own, kept in the Club file.

## Questions

#### Why is Pair Round greyed out?

The reason shows beside it: a result of the last Round is missing, every Round has been paired, or fewer than two players are checked in.

#### How do I take in someone who came after the pairing?

Register and check them in, then **Pair again** before any result is entered. Bragd pairs the Round again from who is here now and logs it with your reason.

#### Can an arbiter change a pairing?

No. An arbiter registers, checks in, pairs, pairs again and enters results. Changing a pairing by hand, deleting a Tournament and rating the games are for a director.

#### Why does Buchholz differ from another program's?

Since March 2026, C.07 caps the score that counts for a player's own unplayed Rounds. A program that still follows the 2024 rules counts a half-point bye at the player's full score.

#### Where are the pairings for the second screen kept?

In Bragd's cache folder, as `pairings.html`, written each time you press **Show on a second screen**. The window shows that page and nothing else.

---
[← Previous: Clubs](clubs.md) · [Next: Look →](../look/README.md)

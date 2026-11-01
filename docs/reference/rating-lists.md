[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Rating lists

Bragd can download rating lists so you can find players by name or ID at check-in. Downloads are opt-in and Bragd shows the size before it fetches anything. Once you agree in the wizard, the Helper fetches the lists you chose every month (see [Clubs](../clubs/clubs.md)). This page records, for FIDE and for each national federation, where the list is, what format it has, what it contains, what terms apply and how often it changes. Every URL was fetched on 2026-11-01.

## Contents

- [The lists Bragd fetches](#the-lists-bragd-fetches)
- [FIDE](#fide)
- [Denmark (DSU)](#denmark-dsu)
- [Other federations](#other-federations)
- [Questions](#questions)

## The lists Bragd fetches

Each is a Source of its own on the Sources page (chess's Databases item), off until you turn it on, fetched whole once a month. The players go into `rating-lists.sqlite` in Bragd's data folder ([Files on disk](files.md)), replacing that list's players from the month before.

| Source | List | About | What Bragd keeps of each player |
|---|---|---|---|
| FIDE | `players_list.zip` | 45 MB | FIDE ID, name, federation, title, standard, rapid and blitz ratings, birth year |
| DSU | the CSV of *Alle medlemmer* | 0.5 MB | DSU number, name, club, title, DSU standard, rapid and blitz ratings, birth year, FIDE ID |
| ECF | `rating-list/csv` | 4.5 MB | ECF code, name, nation, club, title, revised standard, rapid and blitz ratings, FIDE ID |

The others below are not fetched: their file is not one a program can read on a schedule, their address changes each period, or their terms forbid it or ask for agreement first. Adding them is [issue #126](https://github.com/mwo-dk/bragd/issues/126).

## FIDE

FIDE publishes one list for every federation. It is the only list that covers all the countries in the table below, so it is the base for every other source.

| File | Format | Size (zip) | Verified 2026-11-01 |
|---|---|---|---|
| [players_list.zip](https://ratings.fide.com/download/players_list.zip) | Combined list (standard, rapid, blitz), fixed-width TXT | 45.1 MB | 200 |
| [players_list_xml.zip](https://ratings.fide.com/download/players_list_xml.zip) | The same, XML | 51.1 MB | 200 |
| [players_list_legacy.zip](https://ratings.fide.com/download/players_list_legacy.zip) | Combined, without unrated players, TXT | 44.9 MB | 200 |
| [players_list_xml_legacy.zip](https://ratings.fide.com/download/players_list_xml_legacy.zip) | The same, XML | 50.5 MB | 200 |
| [standard_rating_list.zip](https://ratings.fide.com/download/standard_rating_list.zip) | Standard only, TXT | 13.4 MB | 200 |
| [standard_rating_list_xml.zip](https://ratings.fide.com/download/standard_rating_list_xml.zip) | Standard only, XML | 14.5 MB | 200 |
| [rapid_rating_list.zip](https://ratings.fide.com/download/rapid_rating_list.zip) | Rapid only, TXT | 11.4 MB | 200 |
| [rapid_rating_list_xml.zip](https://ratings.fide.com/download/rapid_rating_list_xml.zip) | Rapid only, XML | 12.3 MB | 200 |
| [blitz_rating_list.zip](https://ratings.fide.com/download/blitz_rating_list.zip) | Blitz only, TXT | 7.7 MB | 200 |
| [blitz_rating_list_xml.zip](https://ratings.fide.com/download/blitz_rating_list_xml.zip) | Blitz only, XML | 8.2 MB | 200 |

The download page is [download_lists.phtml](https://ratings.fide.com/download_lists.phtml) (200).

- **Updated:** monthly. The files for the 1 November 2026 list were built on 31 October 2026 (`Last-Modified: Sat, 31 Oct 2026 22:03:26 GMT`). The page archive goes back to February 2015.
- **Licence:** none stated. The page footer says "© 2026 FIDE International Chess Federation. All Rights Reserved." and "No part of this site may be reproduced, stored in a retrieval system or transmitted in any way or by any means ... without the written permission of FIDE International Chess Federation."
- **Size unzipped:** `players_list.zip` holds one file, `players_list_foa.txt`, 319.7 MB with 1,949,164 lines.
- **Encoding:** the first 2,000 bytes are plain ASCII. Lines end in CR LF. Each line is 161 characters.

The first line of `players_list_foa.txt` is the header:

```
ID Number      Name                                                         Fed Sex Tit  WTit OTit           FOA SRtng SGm SK RRtng RGm Rk BRtng BGm BK B-day Flag
```

The columns are fixed-width. Positions count from 1:

| Field | Start | End | Meaning |
|---|---|---|---|
| ID Number | 1 | 15 | FIDE ID |
| Name | 16 | 76 | "Surname, Given names" |
| Fed | 77 | 80 | Federation, three letters |
| Sex | 81 | 84 | M or F |
| Tit | 85 | 89 | Title |
| WTit | 90 | 94 | Women's title |
| OTit | 95 | 109 | Other titles |
| FOA | 110 | 113 | FIDE online arena title |
| SRtng | 114 | 119 | Standard rating |
| SGm | 120 | 123 | Standard games this period |
| SK | 124 | 126 | Standard K-factor |
| RRtng | 127 | 132 | Rapid rating |
| RGm | 133 | 136 | Rapid games |
| Rk | 137 | 139 | Rapid K-factor |
| BRtng | 140 | 145 | Blitz rating |
| BGm | 146 | 149 | Blitz games |
| BK | 150 | 152 | Blitz K-factor |
| B-day | 153 | 158 | Birth year |
| Flag | 159 | 161 | Flags (one letter; the file does not explain them) |

The columns were read from the header and 30 data lines. The meaning of the Flag letters is not written in the file. The XML files were not opened. The XML download page says the fields are the ones in the legend: "STD/SRTNG - Standard rating", "RPD/RRTNG - Rapid rating", "BLZ/BRTNG - Blitz rating".

## Denmark (DSU)

Dansk Skak Union (DSU) has no file called "ratingliste" and no folder of downloads. Its tournament system, `turnering.skak.dk`, makes the list on request. The page [Alle medlemmer](https://turnering.skak.dk/ClubAndMembers/AllMembers) (200) has four buttons: PDF, CSV, RTF and HTML. The CSV button is the one a program can use.

| Item | Value |
|---|---|
| URL | `https://turnering.skak.dk/ClubAndMembers/AllMemberReport?memType=ActiveMembersAjax&format=csv` |
| Verified | 2026-11-01, 200 |
| Served as | `Content-Disposition: attachment; filename=medlemmer.csv`, `Content-Type: plain/txt` (not a standard type) |
| Size | 523 KB, 4,786 players |
| Encoding | ISO-8859-1 (Latin 1), lines end in CR LF |
| Separator | semicolon, every field in double quotes |
| Updated | live: the file is built when you ask. The DSU rules name no publication day. The rules say tournaments "skal tilstræbes beregnet i kronologisk rækkefølge" (should be calculated in date order) |
| Licence | none stated. The page footer says "© 2014-2020 Copyright Dansk Skak Union" |

The first line is `""` (an empty quoted field). The second line is the header:

```
"Nr";"Født";"Alder";"Navn";"Klub";"Titel";"Rat.";"Hur.";"Lyn";"Fide";"Nr";"K"
```

(In the file the `ø` of `Født` is in Latin 1.)

| Column | Meaning |
|---|---|
| Nr | DSU member number, nine digits |
| Født | Birth year |
| Alder | Age this year |
| Navn | Full name, "Given names Surname" |
| Klub | Club name |
| Titel | FIDE title, empty for most players (182 of 4,786 have one) |
| Rat. | DSU standard rating |
| Hur. | DSU rapid rating (hurtigskak) |
| Lyn | DSU blitz rating (lynskak) |
| Fide | FIDE standard rating |
| Nr (second) | FIDE ID |
| K | K-factor (no description on the page; the values seen are 10, 20, 40 and empty for 1,793 players) |

A second CSV exists for Danish FIDE members: the same URL with `memType=ActiveFideMembersAjax` (link found on [Fide medlemmer](https://turnering.skak.dk/ClubAndMembers/ActiveFideMembers), 200). It was not downloaded. Its columns were not read.

Personal data: the file has name, birth year, age and club for every player, which goes beyond name, ID, club and rating.

The list holds the active members only. The rules say a player whose last rated result is more than three years old leaves the list (see the [rating rules, 24 June 2025](https://love.skak.dk/wp-content/uploads/sites/3/Ratingreglementet-20250624.pdf), 200). The rules also set a minimum of 1200. A rating of a returning player is stored and used again.

## Other federations

"FIDE list only" means the federation's own site shows no download that a program can fetch. Where a row says "web page only", a person can look players up but the page has no file or API. "Verified" is the HTTP status of the file or page on 2026-11-01. Federations not listed individually in this table were not searched beyond their home page.

| Country | Federation | List | Format | Licence | Updated | Verified |
|---|---|---|---|---|---|---|
| Germany | DSB (DWZ) | [LV-0-csv.zip](https://www.schachbund.de/download-dwz-daten.html?file=files/wertungsportal/downloads/export/csv/LV-0-csv.zip), the whole DSB, 3.1 MB; one zip per regional association (`LV-1` to `LV-M`) is on the [download page](https://www.schachbund.de/download-dwz-daten.html) | ZIP of `spieler.csv`, `vereine.csv`, `verbaende.csv`, `README.txt`. Comma separated, Windows-1252. Fields: ID, club number, membership number, status (A active, P passive), "Surname,Given name", sex, birth year, last evaluation week, DWZ, index, FIDE rating, title, ID, country, rapid and blitz | The README says: "Eine Veröffentlichung dieser Daten ist nur nach vorheriger Absprache mit dem zuständigen DWZ-Referenten erlaubt!" (publishing needs prior agreement) | Every day at 04:15, DWZ joined with the current FIDE data. 97,712 players on 31 Oct 2026 | 200 (file), 200 (page) |
| England (UK) | ECF | [rating-list/csv](https://rating.englishchess.org.uk/api/rating-list/csv), 4.5 MB, 45,830 rows; also [json](https://rating.englishchess.org.uk/api/rating-list/json) and [sm](https://rating.englishchess.org.uk/api/rating-list/sm) (Swiss Manager, fewer columns) | CSV with a header: ECF code, name, member number, FIDE ID, sex, nation, original and revised standard, rapid and blitz (also online), club code, club name, title | None stated. The [API page](https://rating.englishchess.org.uk/help/api) says "Subject to change without notice currently" and limits use to 10 minutes of processing per day per class C network | "Original" ratings at month end, "revised" ratings nightly | 200 (csv); API page 200 |
| Australia | ACF | [rating-lists page](https://auschess.org.au/rating-lists/) with [sepmst26.txt](https://auschess.org.au/wp-content/uploads/2026/09/sepmst26.txt) (master file, 2.4 MB), [sep26act.txt](https://auschess.org.au/wp-content/uploads/2026/09/sep26act.txt) (active, 226 KB), `sep26quick.txt`, and Vega and Swiss Perfect zips ([vegamast.zip](https://auschess.org.au/wp-content/uploads/2026/09/vegamast.zip) 0.9 MB, [acfmast.zip](https://auschess.org.au/wp-content/uploads/2026/09/acfmast.zip) 1.4 MB) | Fixed-width TXT, CR LF: rating (with reliability mark), games, rank and points columns, state, name. Master file starts `ACF Master File for September 2026`; the first column is the ACF ID | Copyright ACF. The page says that by downloading you agree not to modify, extract or reverse engineer the data, or import it into other systems. Bragd must not import it | Per rating period; the page says "Updated September 5th, 2026". File names carry the month, so the URL changes | 200 (page); files 206 on a range request. A plain HEAD request returned 404 |
| Canada | CFC | [cfcmembers.mde](https://storage.googleapis.com/cfc-public/data/cfcmembers.mde), 9.2 MB | Microsoft Access database (`.mde`) with CFC and FIDE IDs and ratings. Linked from the [Swiss-Manager tools page](https://www.chess.ca/en/organizers/tools/swiss-manager) | None stated | Not stated; the file was last changed on 30 Oct 2026 | 200 |
| New Zealand | NZCF | [2026-3-Alphabetical-20261020.xlsx](https://compete.newzealandchess.co.nz/wp-content/uploads/2026/10/2026-3-Alphabetical-20261020.xlsx) (429 KB) and [2026-3-Active-20261020.xlsx](https://compete.newzealandchess.co.nz/wp-content/uploads/2026/10/2026-3-Active-20261020.xlsx) (80 KB), from the [ratings page](https://compete.newzealandchess.co.nz/new-zealand-ratings-list/). The page also has CSV and JSON buttons whose URLs were not found | XLSX; the file name carries the period and the date, so the URL changes | None stated | Four times a year (1 Jan, 1 Apr, 1 Jul, 1 Oct, per the rating regulations); the page says "Updated 22/10/2026" | 200 (both) |
| Czechia | Šachový svaz ČR (LOK) | [20261001_lok_sm_ascii.zip](https://elo.miramal.com/download/20261001_lok_sm_ascii.zip) (1.4 MB), [20261001_lok_sm_cz.zip](https://elo.miramal.com/download/20261001_lok_sm_cz.zip) (1.4 MB, with diacritics), rapid in `..._rapid_lok_sm_cz.zip` | ZIP of one old binary XLS (`lok_sm_ascii.xls`, 4.6 MB), made for Swiss Manager. Columns not read | None stated | Periodic (list dated 1 Oct 2026, "Aktualizováno: 1. říjen 2026"). File names carry the date | 200 |
| Finland | Suomen Shakkiliitto (tasaselo) | [selolist.txt](http://www.shakki.net/tasaselo/selolist.txt), 784 KB, about 12,000 lines. Page: [shakki.net/tasaselo](https://www.shakki.net/tasaselo/) | Fixed-width TXT, Latin 1, CR LF. Header `Nimi / Seura / Selo / Pelejä / L`. Columns: running number, surname, given name, club, rating, games. No ID | None stated | Irregular; dated 15 Oct 2026 | 200 (the `http` URL redirects once) |
| Spain | FEDA | [Lista Elo FEDA Diciembre 2023](https://feda.org/feda2k16/wp-content/uploads/2023_12_FEDA.xlsx), 3.4 MB; index page [elo-feda](https://feda.org/feda2k16/elo-feda/) | XLSX, one per month. Older months are zips (`2022-04.zip`, 2.8 MB, with a lookup program) | None stated | Monthly until December 2023. The page has no newer list, and the 2024 and 2026 file names I tried returned 404 | 200 (December 2023 file); 404 for later months |
| Spain | FCE (Catalonia) | Web view [escacs.cat/index.php/component/fce?op=25](https://escacs.cat/index.php/component/fce?op=25) | Web page only; no file found | None stated | Not stated | 200 |
| Spain | Basque federation | FIDE list only | Not searched in depth | | | |
| Austria | ÖSB | [chess-results.com/OesbEloSuche.aspx](https://chess-results.com/OesbEloSuche.aspx) | Web search page only; [chess.at/elo.html](https://www.chess.at/elo.html) links to it. The only download on [chess.at](https://www.chess.at/service/downloads.html) is the Elo rules (PDF) | None stated | Not stated | 200 (page); search page not fetched |
| Switzerland | SSB | [Spielersuche](https://swisschess.ch/de/sport/rating-spieler/spielersuche) | Web table only (columns Code, FIDE-ID, Name, Vorname, Liz., Elo, Kat., Sektion). No export | None stated | Not stated | 200 |
| Armenia | Chess Federation of Armenia | [players-ratings](https://www.chessfed.am/am/players-ratings/) | Page with an "Download in .xlsx" button for national and FIDE ratings. The button URL was not found, so a program cannot use it yet | None stated | International list "as of 01/10/2026" | 200 (page) |
| Israel | Israel Chess Federation | [RatingUpdateInfo](https://www.chess.org.il/Players/RatingUpdateInfo.aspx) | Paginated web table only (number, name, rating, change, games) | Site terms page exists; content not read | Update dated 01/10/2026 | 200 |
| France | FFE | FIDE list only | The home page links only to Elo Rapide and to chess-results pages. No download found | | | home page 200 |
| Belgium | FRBE / KBSB | no public list found | The site is a JavaScript app; no file link found | | | home page 200 |
| Netherlands | KNSB | no public list found | The KNSB rules say the rating commissioner compiles a full list "at least four times a year". The viewer site `ratingviewer.nl` is a JavaScript app; no file or API found | | | 200 (viewer home page) |
| Sweden | SSF | no public list found | `member.schack.se` is a JavaScript app; `www.schack.se` did not answer | | | `www.schack.se` not reachable |
| Norway | NSF | no public list found | No rating page found on the home page | | | home page 200 |
| Poland | PZSzach | no public list found | The member and club registers are web pages; no ranking file | | | home page 200 |
| Estonia | Eesti Maleliit | no public list found | The site is a JavaScript app; no file link found | | | home page 200 |
| Greece | Hellenic Chess Federation | no public list found | `www.chessfed.gr` answered 406 to the check | | | not reachable (406) |
| Ukraine | UCF | no public list found | `ucf.org.ua` now redirects to an unrelated site; do not use | | | 200 but wrong site |
| Italy | FSI | no public list found | Not checked beyond the home page | | | home page 200 |
| Singapore | Singapore Chess Federation | FIDE list only | Not checked beyond the home page | | | home page 200 |
| Taiwan | Chinese Taipei Chess Association | FIDE list only | Not checked beyond the home page | | | home page 200 |
| Iran | Iran Chess Federation | FIDE list only | `chess.org.ir` not reachable; `fcf.ir` answered 200, not searched | | | not reachable |
| Argentina | FADA | FIDE list only | The address `fada.org.ar` I tried belongs to another sport; not confirmed | | | 200, wrong site |
| Japan | JCA | FIDE list only | Not reachable by the address tried | | | not reachable |
| South Korea | Korea Chess Federation | FIDE list only | Not reachable by the address tried | | | not reachable |
| Georgia | Georgian Chess Federation | FIDE list only | Not reachable by the address tried | | | not reachable |
| Lithuania | LŠF | FIDE list only | Not reachable by the addresses tried | | | not reachable |
| Latvia | LŠF | FIDE list only | Not reachable by the address tried | | | not reachable |
| Mexico | FEMEXA | FIDE list only | Not reachable by the address tried | | | not reachable |
| Chile, Colombia, Peru | national federations | FIDE list only | Not reachable by the addresses tried | | | not reachable |
| Bolivia, Costa Rica, Cuba, Dominican Republic, Ecuador, Guatemala, Honduras, Nicaragua, Panama, Puerto Rico, El Salvador, Uruguay, Venezuela | national federations | FIDE list only | Not searched; no national list known | | | not checked |

Data from the German, English, Australian and Danish rows has names and birth years or member numbers of private people. Bragd keeps any list it downloads on the machine and does not upload it.

## Questions

#### Why is my federation's list missing?

Most small federations only use the FIDE list, and many of the others publish no file, only a search page. A search page is no use to Bragd, which must fetch a list on a schedule and check players offline. Where a federation has a list but its terms forbid use in other software (Australia) or ask for agreement first (Germany), the federation's permission is needed before Bragd can use it.

#### Is a downloaded list sent anywhere?

No. It stays in `rating-lists.sqlite` on your machine. Searching it at check-in asks nothing of the network.

#### How big are the downloads?

The FIDE combined list is 45 MB zipped and 320 MB unzipped, about 1.9 million lines. The standard-only list is 13 MB. The Danish list is 0.5 MB and the English one 4.5 MB. The wizard and the Sources page show each list's size before anything is downloaded.

#### How much disk does a list take, and how fast is a search?

The whole FIDE list, 1,949,163 players, takes 386 MB in `rating-lists.sqlite`, its name index included (measured on 2026-11-01). The import takes about a minute and a half. A search by name reads only the players whose names match, through the index: a full name such as `magnus carlsen` or a surname such as `nielsen` is answered at once. A single letter matches most of the list and takes longest.

#### How often does Bragd fetch?

Once a month, once you tick the list in the wizard or turn it on under Sources. FIDE builds its files on the last day of the month. **Fetch now** on the Sources page fetches one at once.

#### Can I use a list Bragd does not know?

Not yet. Bragd reads only the three lists above. Members a list does not have can be added to a Club by hand, with their IDs ([Clubs](../clubs/clubs.md)).

---
[← Previous: Privacy](privacy.md) · [Next: Swiss pairing →](swiss-pairing.md)

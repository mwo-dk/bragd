[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Rating lists

Bragd can download rating lists so you can find players by name or ID at check-in. Downloads are opt-in and Bragd shows the size before it fetches anything. Once you agree in the wizard, the Helper fetches the lists you chose every month (see [Clubs](../clubs/clubs.md)). This page records, for FIDE and for each national federation, where the list is, what format it has, what it contains, what terms apply and how often it changes. Every URL was fetched on 2026-11-01.

## Contents

- [The lists Bragd fetches](#the-lists-bragd-fetches)
- [FIDE](#fide)
- [Denmark (DSU)](#denmark-dsu)
- [Canada (CFC)](#canada-cfc)
- [Finland (SShL)](#finland-sshl)
- [New Zealand (NZCF)](#new-zealand-nzcf)
- [Lists Bragd does not fetch](#lists-bragd-does-not-fetch)
- [Other federations](#other-federations)
- [Questions](#questions)

## The lists Bragd fetches

Each is a Source of its own on the Sources page (chess's Databases item), off until you turn it on, fetched whole once a month. The players go into `rating-lists.sqlite` in Bragd's data folder ([Files on disk](files.md)), replacing that list's players from the month before.

| Source | List | About | What Bragd keeps of each player |
|---|---|---|---|
| FIDE | `players_list.zip` | 45 MB | FIDE ID, name, federation, title, standard, rapid and blitz ratings, birth year |
| DSU | the CSV of *Alle medlemmer* | 0.5 MB | DSU number, name, club, title, DSU standard, rapid and blitz ratings, birth year, FIDE ID |
| ECF | `rating-list/csv` | 4.5 MB | ECF code, name, nation, club, title, revised standard, rapid and blitz ratings, FIDE ID |
| CFC | `cfc-ratings.csv` | 5.4 MB | CFC ID, name, standard and rapid ratings, FIDE ID |
| SShL | `selolista.csv` | 0.75 MB | SShL ID, name, nationality, club, title, national standard and rapid ratings, birth year, FIDE ID |
| NZCF | `nz_ratings.json` | 0.9 MB | NZCF ID, name, federation, club, title, NZCF standard and rapid ratings, birth year, FIDE ID |

The others are not fetched, each for the reason in [Lists Bragd does not fetch](#lists-bragd-does-not-fetch).

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

## Canada (CFC)

| Item | Value |
|---|---|
| URL | [`https://files.chess.ca/data/cfc-ratings.csv`](https://files.chess.ca/data/cfc-ratings.csv), from the Data Files box of the [organisers page](https://www.chess.ca/en/organizers/) |
| Verified | 2026-11-01, 200 |
| Size | 5.4 MB, 96,896 rows; 55,750 with a CFC rating, 10,654 with a FIDE ID |
| Encoding | UTF-8, lines end in CR LF, comma separated, a field holding a comma is quoted |
| Updated | Not stated; the file was last changed on 30 Oct 2026 |
| Licence | None stated. `robots.txt` allows every agent |

The header is `cfc_id,name_first,name_last,cfc_expiry,cfc_rating,cfc_rating_rapid,fide_id,city,province,sex`. The second row is not a player: its `cfc_id` is `-100` and its `name_first` holds the list's date. A rating or FIDE ID of `0` means none. Lapsed members stay in the list. The organisers page labels the link `cfc-ratings.zip`, which answers 404; the file is the `.csv` beside it.

## Finland (SShL)

| Item | Value |
|---|---|
| URL | [`https://www.shakki.net/selo/selolista.csv`](https://www.shakki.net/selo/selolista.csv), "koko lista CSV-muodossa" on the [rating page](https://www.shakki.net/selo/) |
| Verified | 2026-11-01, 200 |
| Size | 740 KB, 12,070 players; 3,527 with a FIDE ID |
| Encoding | ISO-8859-1 (Latin 1), lines end in CR LF, `;` separated, every line ends in a `;` |
| Updated | Irregular; the national list was dated 30 Oct 2026, the rapid list 23 Oct 2026 |
| Licence | None stated |

The header is `id;fide_id;titteli;sukunimi;etunimi;seura;selo;selopelejä;selovuosi;pelo;pelopelejä;pelovuosi;elo;sukupuoli;kansallisuus;syntvuosi;lisenssi;`. `selo` is the national standard rating and `pelo` the rapid one; `elo` is the FIDE rating. `titteli` holds Finnish titles (such as ME and KvM) as well as FIDE's.

## New Zealand (NZCF)

| Item | Value |
|---|---|
| URL | [`https://compete.newzealandchess.co.nz/wp-content/uploads/2024/10/nz_ratings.json`](https://compete.newzealandchess.co.nz/wp-content/uploads/2024/10/nz_ratings.json), the file the [ratings page](https://compete.newzealandchess.co.nz/new-zealand-ratings-list/) draws its table from |
| Verified | 2026-11-01, 200 |
| Size | 871 KB, 4,036 players; 2,445 with a FIDE ID |
| Encoding | UTF-8, one line |
| Updated | Four times a year; the file is replaced in place (last changed 21 Oct 2026) |
| Licence | None stated. `robots.txt` asks for 10 seconds between requests; Bragd makes one a month |

The file is an array of objects whose values are all strings: `surname`, `name`, `fed`, `year`, `title`, `fide`, `nzcf`, `club`, `nzcf_standard`, `nzcf_rapid`, `fide_standard`, `fide_rapid`. An empty value or `0` means none. The CSV and JSON buttons on the page make their file in the browser from this one. The address names a 2024 upload folder: if the NZCF moves the file, the fetch fails and the Sources page says so.

## Lists Bragd does not fetch

Checked again on 2026-11-01.

- **Germany (DSB):** the README in the zip says publishing the data needs prior agreement with the DWZ referee. Bragd waits for the DSB's agreement.
- **Australia (ACF):** the terms forbid importing the files into any other database or program.
- **Catalonia (FCE):** the legal notice allows private download but forbids transforming the content, which an import does.
- **Sweden (SSF):** the API's terms allow it, but the date is part of the address and a wrong date gives an empty list. Bragd needs to learn the list dates first.
- **Czechia:** the stable file is an old binary Excel file, which Bragd cannot read.
- **Austria, Colombia, Japan:** the file name carries the period, and Austria's and Japan's are Excel and PDF.
- **Armenia:** the download is a form post, with no address to fetch.
- **Switzerland, Netherlands, Belgium, Norway, Poland, Estonia, Israel, Greece, Georgia, Latvia:** a search page or a JavaScript app only, with no file or documented API.
- **Lithuania, Chile, Peru, Mexico, South Korea, Iran:** no list found, or the federation's site was not found.

## Other federations

"FIDE list only" means the federation's own site shows no download that a program can fetch. Where a row says "web page only", a person can look players up but the page has no file or API. "Verified" is the HTTP status of the file or page on 2026-11-01. Federations not listed individually in this table were not searched beyond their home page.

| Country | Federation | List | Format | Licence | Updated | Verified |
|---|---|---|---|---|---|---|
| Germany | DSB (DWZ) | [LV-0-csv.zip](https://www.schachbund.de/download-dwz-daten.html?file=files/wertungsportal/downloads/export/csv/LV-0-csv.zip), the whole DSB, 3.1 MB; one zip per regional association (`LV-1` to `LV-M`) is on the [download page](https://www.schachbund.de/download-dwz-daten.html) | ZIP of `spieler.csv`, `vereine.csv`, `verbaende.csv`, `README.txt`. Comma separated, Windows-1252. Fields: ID, club number, membership number, status (A active, P passive), "Surname,Given name", sex, birth year, last evaluation week, DWZ, index, FIDE rating, title, ID, country, rapid and blitz | The README says: "Eine Veröffentlichung dieser Daten ist nur nach vorheriger Absprache mit dem zuständigen DWZ-Referenten erlaubt!" (publishing needs prior agreement) | Every day at 04:15, DWZ joined with the current FIDE data. 97,712 players on 31 Oct 2026 | 200 (file), 200 (page) |
| England (UK) | ECF | [rating-list/csv](https://rating.englishchess.org.uk/api/rating-list/csv), 4.5 MB, 45,830 rows; also [json](https://rating.englishchess.org.uk/api/rating-list/json) and [sm](https://rating.englishchess.org.uk/api/rating-list/sm) (Swiss Manager, fewer columns) | CSV with a header: ECF code, name, member number, FIDE ID, sex, nation, original and revised standard, rapid and blitz (also online), club code, club name, title | None stated. The [API page](https://rating.englishchess.org.uk/help/api) says "Subject to change without notice currently" and limits use to 10 minutes of processing per day per class C network | "Original" ratings at month end, "revised" ratings nightly | 200 (csv); API page 200 |
| Australia | ACF | [rating-lists page](https://auschess.org.au/rating-lists/) with [sepmst26.txt](https://auschess.org.au/wp-content/uploads/2026/09/sepmst26.txt) (master file, 2.4 MB), [sep26act.txt](https://auschess.org.au/wp-content/uploads/2026/09/sep26act.txt) (active, 226 KB), `sep26quick.txt`, and Vega and Swiss Perfect zips ([vegamast.zip](https://auschess.org.au/wp-content/uploads/2026/09/vegamast.zip) 0.9 MB, [acfmast.zip](https://auschess.org.au/wp-content/uploads/2026/09/acfmast.zip) 1.4 MB) | Fixed-width TXT, CR LF: rating (with reliability mark), games, rank and points columns, state, name. Master file starts `ACF Master File for September 2026`; the first column is the ACF ID | Copyright ACF. The page says that by downloading you agree not to modify, extract or reverse engineer the data, or import it into other systems. Bragd must not import it | Per rating period; the page says "Updated September 5th, 2026". File names carry the month, so the URL changes | 200 (page); files 206 on a range request. A plain HEAD request returned 404 |
| Canada | CFC | Fetched: see [Canada (CFC)](#canada-cfc). Also [cfcmembers.mde](https://storage.googleapis.com/cfc-public/data/cfcmembers.mde) (9.2 MB, Microsoft Access) and [tdlist.txt](https://storage.googleapis.com/cfc-public/data/tdlist.txt) (5.3 MB), linked from the [organisers page](https://www.chess.ca/en/organizers/) | CSV | None stated | See its section | 200 |
| New Zealand | NZCF | Fetched: see [New Zealand (NZCF)](#new-zealand-nzcf). The XLSX files on the [ratings page](https://compete.newzealandchess.co.nz/new-zealand-ratings-list/) carry the period in their names | JSON | None stated | Four times a year | 200 |
| Czechia | Šachový svaz ČR (LOK) | [lok_sm_cz.xls](https://elo.miramal.com/download/lok_sm_cz.xls) (4.7 MB), overwritten each period; dated zips such as [20261001_lok_sm_cz.zip](https://elo.miramal.com/download/20261001_lok_sm_cz.zip) beside it. [chess.cz](https://www.chess.cz/sachovy-svaz-cr/elo/) links only here and to FIDE | Old binary Excel (BIFF, code page 1250), made for Swiss Manager. Columns: ID_no, Name, ClubName, BirthDay (`00.00.YYYY`), Sex, Rtg_nat, Rtg_int, Title, FIDE_no, Fed, ClubNo, Status | None stated | Classical list each 1 February, rapid each 1 November; the list shown is dated 1 Oct 2026 | 200 |
| Finland | Suomen Shakkiliitto (SShL) | Fetched: see [Finland (SShL)](#finland-sshl). The older [selolist.txt](http://www.shakki.net/tasaselo/selolist.txt) is fixed-width with no FIDE ID | CSV | None stated | See its section | 200 |
| Spain | FEDA | [Lista Elo FEDA Diciembre 2023](https://feda.org/feda2k16/wp-content/uploads/2023_12_FEDA.xlsx), 3.4 MB; index page [elo-feda](https://feda.org/feda2k16/elo-feda/) | XLSX, one per month. Older months are zips (`2022-04.zip`, 2.8 MB, with a lookup program) | None stated | Monthly until December 2023. The page has no newer list, and the 2024 and 2026 file names I tried returned 404 | 200 (December 2023 file); 404 for later months |
| Spain | FCE (Catalonia) | [EloActualStd.xls](https://escacs.cat/arxielo/EloActualStd.xls) and [EloActualRpd.xls](https://escacs.cat/arxielo/EloActualRpd.xls) (580 KB each), linked from [the Elo page](https://escacs.cat/index.php/component/fce?op=25) | Tab-separated text despite the name, Latin 1: FCE code, name, sex, K, Elo, title, category, club. No FIDE ID | The [legal notice](https://escacs.cat/index.php/avis-legal) allows download "pel seu us privat i sense cap finalitat comercial" and forbids "transformació, distribució ... qualsevol altre forma d'explotació" | Monthly (last changed 31 Oct 2026) | 200 |
| Spain | Basque federation | FIDE list only | Not searched in depth | | | |
| Austria | ÖSB | [chess-results.com/OesbEloSuche.aspx](https://chess-results.com/OesbEloSuche.aspx) offers the list as a zipped Excel file, e.g. [Eloliste_20261001.zip](https://chess-results.com/download/elolisten/Eloliste_20261001.zip) | Excel in a zip; the file name carries the date | None stated | Per rating period | 200 |
| Switzerland | SSB | [Spielersuche](https://swisschess.ch/de/sport/rating-spieler/spielersuche) | Web table only (columns Code, FIDE-ID, Name, Vorname, Liz., Elo, Kat., Sektion). No export | None stated | Not stated | 200 |
| Armenia | Chess Federation of Armenia | [players-ratings](https://www.chessfed.am/am/players-ratings/) | The "Download in .xlsx" buttons post a form (`operation=international_xls_export`); there is no address to fetch | None stated | International list "as of 01/10/2026" | 200 (page) |
| Israel | Israel Chess Federation | [RatingUpdateInfo](https://www.chess.org.il/Players/RatingUpdateInfo.aspx) | Paginated web table only (number, name, rating, change, games) | Site terms page exists; content not read | Update dated 01/10/2026 | 200 |
| France | FFE | FIDE list only | The home page links only to Elo Rapide and to chess-results pages. No download found | | | home page 200 |
| Belgium | FRBE / KBSB | no public list found | The site is a JavaScript app; no file link found | | | home page 200 |
| Netherlands | KNSB | no public list found | The KNSB rules say the rating commissioner compiles a full list "at least four times a year". The viewer site `ratingviewer.nl` is a JavaScript app; no file or API found | | | 200 (viewer home page) |
| Sweden | SSF | Public API, [documented](https://member.schack.se/swagger-ui/index.html): `https://member.schack.se/public/api/v1/ratinglist/federation/date/{YYYY-MM-DD}/ratingtype/{1,6,7}/category/0` | JSON, UTF-8: SSF ID, names, birth year, FIDE ID, club, standard, rapid and blitz ratings | The [terms](https://halvarsson.no-ip.com/wordpressssf/publikt-api/anvandarvillkor/) allow use in other products, commercial use included, if the source is named. The date is part of the address; `2026-11-01` answered with 3,319 players, a date with no list answers `[]` | Monthly | 200 |
| Norway | NSF | no public list found | No rating page found on the home page | | | home page 200 |
| Poland | PZSzach | no public list found | The member and club registers are web pages; no ranking file | | | home page 200 |
| Estonia | Eesti Maleliit | no public list found | The site is a JavaScript app; no file link found | | | home page 200 |
| Greece | Hellenic Chess Federation | no public list found | [chessfed.gr/elo](https://www.chessfed.gr/elo) is a search page; the earlier 406 came from the checker's user agent | | | 200 |
| Ukraine | UCF | no public list found | `ucf.org.ua` now redirects to an unrelated site; do not use | | | 200 but wrong site |
| Italy | FSI | no public list found | Not checked beyond the home page | | | home page 200 |
| Singapore | Singapore Chess Federation | FIDE list only | Not checked beyond the home page | | | home page 200 |
| Taiwan | Chinese Taipei Chess Association | FIDE list only | Not checked beyond the home page | | | home page 200 |
| Iran | Iran Chess Federation | no public list found | [fcf.ir](https://fcf.ir/fa) has no rating page; `chess.org.ir` does not resolve | | | 200 |
| Argentina | FADA | FIDE list only | The address `fada.org.ar` I tried belongs to another sport; not confirmed | | | 200, wrong site |
| Japan | JCA | Monthly PDF, e.g. [2026-11-01.pdf](https://japanchess.org/wp-content/uploads/2026/10/2026-11-01.pdf) | PDF; the address carries the month | | Monthly | 200 |
| South Korea | Korea Chess Federation | no public list found | [kcf.or.kr](https://kcf.or.kr/) has no rating page | | | 200 |
| Georgia | Georgian Chess Federation | no public list found | [geochess.ge](https://geochess.ge/) has rating pages but no download | | | 200 |
| Lithuania | LŠF | FIDE list only | `lsf.lt` belongs to another sport; `sachmatai.lt` is "under reconstruction" | | | wrong site |
| Latvia | LŠF | no public list found | [sahafederacija.lv](https://www.sahafederacija.lv/) is a JavaScript app | | | 200 |
| Mexico | FEMEXA | FIDE list only | Not reachable by the address tried | | | not reachable |
| Colombia | Federación Colombiana de Ajedrez | [elo-2](https://federacioncolombianadeajedrez.com/elo-2/) lists monthly files, e.g. `ELOOct.csv` (15.3 MB) and `Elo_Nac_COL_octubre_2026.xlsx` | CSV and XLSX; the month is in each name | None stated | Monthly | 200 |
| Chile, Peru | national federations | FIDE list only | Not reachable: `ajedrezchile.cl` now leads to an unrelated site; the Peruvian addresses tried do not resolve | | | not reachable |
| Bolivia, Costa Rica, Cuba, Dominican Republic, Ecuador, Guatemala, Honduras, Nicaragua, Panama, Puerto Rico, El Salvador, Uruguay, Venezuela | national federations | FIDE list only | Not searched; no national list known | | | not checked |

Data from the German, English, Canadian, Finnish, New Zealand, Australian and Danish rows has names and birth years or member numbers of private people. Bragd keeps any list it downloads on the machine and does not upload it.

## Questions

#### Why is my federation's list missing?

Most small federations only use the FIDE list, and many of the others publish no file, only a search page. A search page is no use to Bragd, which must fetch a list on a schedule and check players offline. Where a federation has a list but its terms forbid use in other software (Australia, Catalonia) or ask for agreement first (Germany), the federation's permission is needed before Bragd can use it.

#### Is a downloaded list sent anywhere?

No. It stays in `rating-lists.sqlite` on your machine. Searching it at check-in asks nothing of the network.

#### How big are the downloads?

The FIDE combined list is 45 MB zipped and 320 MB unzipped, about 1.9 million lines. The standard-only list is 13 MB. The Danish list is 0.5 MB, the Finnish 0.75 MB, the New Zealand 0.9 MB, the English 4.5 MB and the Canadian 5.4 MB. The wizard and the Sources page show each list's size before anything is downloaded.

#### How much disk does a list take, and how fast is a search?

The whole FIDE list, 1,949,163 players, takes 386 MB in `rating-lists.sqlite`, its name index included (measured on 2026-11-01). The import takes about a minute and a half. A search by name reads only the players whose names match, through the index: a full name such as `magnus carlsen` or a surname such as `nielsen` is answered at once. A single letter matches most of the list and takes longest.

#### How often does Bragd fetch?

Once a month, once you tick the list in the wizard or turn it on under Sources. FIDE builds its files on the last day of the month. **Fetch now** on the Sources page fetches one at once.

#### Can I use a list Bragd does not know?

Not yet. Bragd reads only the six lists above. Members a list does not have can be added to a Club by hand, with their IDs ([Clubs](../clubs/clubs.md)).

---
[← Previous: Privacy](privacy.md) · [Next: Swiss pairing →](swiss-pairing.md)

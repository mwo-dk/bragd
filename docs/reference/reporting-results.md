[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Reporting results

After a rated Tournament the director sends the results to a rating body: FIDE, a national federation, or both. This page records, for FIDE, for chess-results.com and for the federation of each country Bragd has a language for, where results go, the file the body takes, who sends it, the terms, and whether there is an upload API. Every page was read on 2026-11-19. How to do it from Bragd, country by country, is in [Reporting results to your federation](../clubs/reporting-results.md).

## Contents

- [What Bragd writes](#what-bragd-writes)
- [At a glance](#at-a-glance)
- [FIDE](#fide)
- [chess-results.com](#chess-resultscom)
- [Federations with a written procedure](#federations-with-a-written-procedure)
- [Federations with no written procedure found](#federations-with-no-written-procedure-found)
- [Formats Bragd does not write](#formats-bragd-does-not-write)
- [Questions](#questions)

## What Bragd writes

| Export | Format | Who reads it |
|---|---|---|
| **FIDE report (TRF)** | FIDE's Tournament Report File, TRF-2026 | FIDE's rating server, through a federation's rating officer; every federation below that names TRF or "the FIDE text format"; pairing programs |
| **Games (PGN)** | PGN, one game per pairing, no moves | Denmark's tournament system, Spain's FIDE reports, Uruguay's rating officer |
| **Standings and crosstable (HTML)** | One static page | People: a federation that asks for a crosstable by email |

No federation below publishes an upload API, and Bragd uploads nothing. The director sends the file.

## At a glance

"Checked" says what the federation's own pages gave on 2026-11-19: **read** means the rule was read in the federation's own document; **summary** means only a search summary or a regional copy was seen; **none** means no procedure was found. "TRF" also covers the names "FIDE report file", "Krause file" and Swiss-Manager's `FIDE_Export` text file, which are the same format.

| Country | Body | Where results go | File | Who sends | Deadline | API | Checked |
|---|---|---|---|---|---|---|---|
| (every country) | FIDE | the federation's FIDE rating officer, who uploads to the FIDE rating server | TRF | chief arbiter | in time for the monthly list | none | read |
| Argentina | FADA | "Pedido y computo de Torneos" on the FADA site | not stated | not stated | not stated | none | summary |
| Armenia | Chess Federation of Armenia | not found | not found | not found | not found | none | none |
| Australia | ACF | FIDE rating officer for FIDE events; national route not found | not found | not found | not found | none | none |
| Austria | ÖSB | chess-results.com upload with the Swiss-Manager file; other programs send TRF to the Elo referee | Swiss-Manager file or TRF | organiser | 2 days | none | read (2011 rules) |
| Belgium | RBCF (FRBE-KBSB) | email; the national Elo is archived since July 2026, FIDE Elo is the ranking | not set publicly | organiser | 3 days | none | summary |
| Bolivia | national federation | not found | not found | not found | not found | none | none |
| Canada | CFC | the CFC office | `.ctr` and `.tms` files | organiser | "as soon as possible" | none | read |
| Chile | AJEFECH | site unreachable | not found | not found | not found | none | none |
| Colombia | FECODAZ | not stated on the rating page | not found | not found | not found | none | summary |
| Costa Rica | FCA | procedure in a regulation PDF not read | not found | not found | not found | none | summary |
| Cuba | national federation | not found | not found | not found | not found | none | none |
| Czechia | ŠSČR (LOK) | the LOK processor; FIDE events through the FIDE rating server | Swiss-Manager data file only | organiser | 7 days | none | read |
| Denmark | DSU | turnering.skak.dk, typed in; by agreement, email to rating@skak.dk | web entry; games as PGN | organiser | 7 days, 3 if FIDE-rated | none | read |
| Dominican Republic | FDA | not found | Swiss-Manager (one event notice) | not found | not found | none | none |
| Ecuador | national federation | not found | not found | not found | not found | none | none |
| El Salvador | FSA | site unreachable | not found | not found | not found | none | none |
| England | ECF | the League Management System (LMS), which sends a rating file; or a rating file to the ECF | ECF rating file (`.txt` or Excel); its layout is not public | organiser or results officer | collected on the 7th, 14th, 21st and 28th | none | read (layout not public) |
| Estonia | Eesti Maleliit | electronic report to the federation | not found | not found | 30 days (old text) | none | summary |
| Finland | SShL | the competition report form to SShL | not stated | chief arbiter | not stated | none | read |
| France | FFE | homologation from the director's FFE account page | not stated (Papi is used) | arbiter | not found | none | summary |
| Georgia | Georgian Chess Federation | not found; events go up on chess-results.com | not found | not found | not found | none | none |
| Germany | DSB (DWZ) | the regional association's rating officer, into the DSB's DeWIS; FIDE events to elo@schachbund.com | not published | not found | not found | read-only | none |
| Greece | Hellenic Chess Federation | "Submit tournament for evaluation", arbiter login on chessfed.net | not found | arbiter | not found | none | summary |
| Guatemala | national federation | not found; the federation uploads to chess-results.com | not found | federation | not found | none | none |
| Honduras | FENAH | not found | not found | not found | not found | none | none |
| Iran | Iran Chess Federation | not found (`fcf.ir` serves another organisation) | not found | not found | not found | none | none |
| Israel | Israel Chess Federation | not found | not found | not found | not found | none | none |
| Italy | FSI | the TORO web system, or email to fsi@federscacchi.it | Vega `.elo` file or TRF | arbiter | 8 days | none | read (FSI guide) |
| Japan | JCA | email to japanchess.rating@gmail.com | the JCA template, or the chess-results.com link | organiser | by the 20th for that month | none | read |
| Latvia | LŠF | not found | not found | not found | not found | none | none |
| Lithuania | LŠF | FIDE registration form to the rating administrator | `.docx` form; results file not stated | organiser | registration 5 days ahead (30 with norms) | none | read |
| Mexico | FENAMAC | site unreachable | Swiss-Manager (search summaries) | not found | not found | none | none |
| Netherlands | KNSB | the KNSB calendar and the OLA module, to the Bondsbureau | "a form fit for processing", not specified | organiser | 3 months | none | read |
| New Zealand | NZCF | the NZCF secretary and rating officer | electronic, in a format the rating officer decides | arbiter | 3 weeks | none | summary |
| Nicaragua | FENICA | not found | not found | not found | not found | none | none |
| Norway | NSF | email to turneringsrapport@sjakk.no (Grand Prix rules) | Turneringservice files "or other known formats" | tournament director | the next day | none | read (Grand Prix only) |
| Panama | FPA | not found; the federation uploads to chess-results.com | not found | federation | not found | none | none |
| Peru | national federation | site unreachable | not found | not found | not found | none | none |
| Poland | PZSzach | the Central Register; format not found | not found | chief arbiter | fees within 7 days (summary) | none | summary |
| Puerto Rico | national federation | not found; the federation uploads to chess-results.com | not found | federation | not found | none | none |
| Scotland | Chess Scotland | email to grading@chessscotland.com for games outside Scotland; domestic route not found | not stated | player (outside Scotland) | not stated | none | read |
| Singapore | SCF | not found | not found | not found | not found | none | none |
| South Korea | Korea Chess Federation | site not found | not found | not found | not found | none | none |
| Spain | FEDA | email to informes@feda.org, through the regional federation | TRF (Krause), the IT3 form and PGN | arbiter | 5 days before the list | none | read (FIDE only) |
| Spain: Basque Country | Basque federation | site not found; FEDA's rules apply to FIDE events | see Spain | see Spain | see Spain | none | none |
| Spain: Catalonia | FCE | not found for the Catalan Elo; FEDA's rules apply to FIDE events | not found | not found | not found | none | none |
| Sweden | SSF | the "Anmäl ELO-turnering" form in member.schack.se | web form | the registrant, with that right in the member system | not stated | none (the public API only reads) | read |
| Switzerland | SSB | national list: the Result Submission Center; FIDE: TRF to the rating officer | CSV from Swiss-Manager or Swiss-Chess (no public spec); TRF | organiser; chief arbiter for FIDE | 7 days | none | read (2013 and 2017 texts) |
| Taiwan | CTCA | site not found | not found | not found | not found | none | none |
| Ukraine | UCF | email to ukrchessrating@gmail.com (FIDE), ukrchessrating@ukr.net (national) | TRF | not stated | not stated | none | read |
| Uruguay | FUA | the rating officer | report with the PGN file | not stated | not stated | none | summary |
| Venezuela | FVA | site unreachable | not found | not found | not found | none | none |
| Wales | Welsh Chess Union | the zone rating officers on the [grading page](https://www.welshchessunion.uk/Grading/index.php) | not stated | not stated | not stated | none | read |

## FIDE

| Item | Value |
|---|---|
| Rules | [FIDE Rating Regulations](https://handbook.fide.com/chapter/B022024), in force since 1 January 2024 (200) |
| Format | [TRF-2026](https://handbook.fide.com/files/handbook/TRF26.pdf), approved 12 May 2025, applied from 1 September 2025 (200). The older [TRF16](https://fide.com/FIDE/handbook/C04Annex2_TRF16.pdf) is still online (200) |
| Example | Neither spec has a whole example file, only single records. Bragd's TRF is tested by reading it back |
| Who sends | Clause 9.1: "The Chief Arbiter of a FIDE registered tournament must provide the tournament report (TRF file) to the Rating Officer of the federation", and "the Rating Officer shall be responsible for uploading the TRF file to the FIDE Rating Server" |
| Deadline | In time for the monthly list the tournament is registered for; if five days or less remain in the month, the next list. A tournament not in by the third list after it ends is not rated |
| Terms | The federation registers the tournament first (3 days ahead; 30 days when players above 2700, or women above 2500, take part) and pays the rating fees. Hybrid events also send the full PGN |
| API | None. [ratings.fide.com](https://ratings.fide.com) has no upload; only rating officers reach the rating server |

TRF-2026 also has records for a national rating beside FIDE's: the federation's code in columns 1 to 3, and record `172` naming the main rating. Bragd's TRF does not write them yet.

## chess-results.com

| Item | Value |
|---|---|
| How | Uploaded from Swiss-Manager: "Internet/ Automatic upload to chess-results.com" ([FAQ item 24](https://chess-results.com/FAQ.aspx?lan=1&id=5791), 200) |
| Format | Swiss-Manager's own file. The FAQ names no TRF import |
| Who | The organiser who made the Swiss-Manager file; each tournament needs its own database key |
| Terms | Not found: the "Terms of use" link leads to an imprint page |
| API | None published |

Austria, Japan and many federations that publish nothing of their own use chess-results.com. Bragd cannot upload there: it would need Swiss-Manager's file format, which is not published.

## Federations with a written procedure

Quotes are from the federation's own document, with an English translation.

### Denmark (DSU)

[EMT rules, April 2023](https://love.skak.dk/wp-content/uploads/sites/3/emt-reglementet-pr-april-2023.pdf) (200), §5: "Indberetning af turneringen via Dansk Skak Unions turneringssystem senest 7 dage efter sidste runde, dog senest 3 dage efter sidste runde for Fide-ratede turneringer." (Report the tournament through the DSU tournament system within 7 days of the last round, or 3 days for FIDE-rated tournaments.) The tournament is created in [turnering.skak.dk](https://turnering.skak.dk) 30 days ahead, 40 if FIDE-rated. The system sends FIDE-rated tournaments on to FIDE. By prior agreement with the rating officer, a report can go to rating@skak.dk instead, with the name, dates, arbiters, system, the players with member numbers and, for FIDE, FIDE IDs, ratings, opponents and scores (§6). No file format is published.

### England (ECF)

Clubs and leagues run events in the ECF's League Management System. LMS collects each event's rating file "4 times a month – on the 7th/14th/21st/28th" for the list on the 1st ([LMS & Auto Grading](https://www.englishchess.org.uk/wp-content/uploads/2021/12/LMS-Auto-Grading.pdf), 200). Events outside LMS send a rating file, `.txt` or Excel, which the ECF's checker program reads. The pages that describe the file's layout, such as [Sending Results for ECF Rating](https://lms.englishchess.org.uk/lms/node/207131), answer 403; the rating system's help has moved to `rating-sbox.englishchess.org.uk`, which answers with a browser check. A file with a new player is rejected until the player has an ECF code. [Rules for acceptance](https://www.englishchess.org.uk/?p=63156) (200): "Organisers must make players in their competitions aware that this data will be forwarded to the ECF."

### Austria (ÖSB)

[ÖSB Elo guidelines](https://www.chess.at/downloads/oesb_elo_richtlinien_oesb.pdf), dated 15 February 2011 (200): "Das Turnierergebnis muss innerhalb von zwei Tagen nach Beendigung der Veranstaltung mit dem Turnierfile über Internet eingespielt werden." (The result must be uploaded with the tournament file within two days of the end.) Tournaments not paired with Swiss-Manager send their data "in dem von der FIDE definierten Textformat" (in FIDE's text format, TRF) to the FIDE referee. Fees in that text: €2.50 a player for a Swiss, €0.40 a game for the national Elo, €100 for late results. Registration five weeks ahead. No newer version was found.

### Switzerland (SSB)

The national list takes results through the Result Submission Center, `rsc.swisschess.ch`, typed in or as the CSV file exported from Swiss-Manager ("Extras → Eloref. Datei SUI") or Swiss-Chess 8.82 or newer (RSC manual, 6 December 2017). No spec of that CSV was found. For FIDE, the SSB's 2013 summary: "Die TRF-Datei ist innerhalb von 7 Tagen nach Turnierende an den Rating Officer des Landes zu senden." (The TRF goes to the national rating officer within 7 days of the end.) The FIDE registration form is due 30 days ahead ([SSB page](https://www.swisschess.ch/de/sport/rating-spieler/anmeldung-fuer-fide-wertung), 200).

### Netherlands (KNSB)

[KNSB rating regulations](https://schaakbond.nl/wp-content/uploads/2024/09/Ratingcommissie-KNSB-ratingreglement.pdf), in force 1 October 2024 (200), C.5.1: results reach the Bondsbureau "binnen drie maanden na beëindiging van de wedstrijd" (within three months of the end); C.5.4: "in een voor verwerking (codering) geschikte vorm" (in a form fit for processing). The organiser registers the event in the KNSB calendar and gets an OLA login ([OLA manual](https://schaakbond.nl/wp-content/uploads/2024/09/Handleiding-OLA.pdf), 200). The OLA manual describes a CSV import of players, not of results.

### Belgium (RBCF, FRBE-KBSB)

`frbe-kbsb.be` moves to [blog.frbe-kbsb-ksb.be](https://blog.frbe-kbsb-ksb.be/fide-homologation/) (301, then 200), which says: "The National ELO rating system has been archived since July, 2026" and "Final tournament results need to be submitted for reporting at the latest 3 days after the tournament ended." The regulations say the organiser sends them by email and that the board decides which programs are accepted. FIDE reporting is built into the SWAR pairing program. Until June 2027 the federation pays the rating costs of registration.

### Spain (FEDA)

FEDA Circular 06-2024, read in the [Valencian federation's copy](https://www.facv.org/wp-content/uploads/2024/04/C06-Informes_y_Eval_Elo_2024.pdf) (200), covers FIDE rating only. C.2: "Los informes de Torneos para su evaluación y su documentación complementaria deberán ser remitidos únicamente por e-mail a la FEDA, a la dirección: (informes@feda.org)." (Reports go only by email to informes@feda.org.) A report holds the Krause (TRF) file, the IT3 form and a PGN file, and goes through the regional federation where the event is played (C.4). Registration 7 days ahead, 30 with norms; €5 a day for late registration; a fee a player by rating band. A report must arrive 5 days before the list; more than 60 days late, it is not rated. The arbiter needs a current FEDA licence. No rule for a national FEDA Elo report was found: FEDA's last national list is from December 2023 ([Rating lists](rating-lists.md)).

### Italy (FSI)

[FSI guide to tournament homologation, 2025](https://www.federscacchi.com/caf/images/CAF/Allegati/Guida_omologazione_dei_tornei_2025.pdf) (200): "se si utilizza Vega inviare il file nometorneo.elo" (with Vega, send `name.elo`) and "se tutti i giocatori sono in possesso di Elo FIDE, inviare il file FIDE_Export_nometorneo.txt" (if every player has a FIDE rating, send Swiss-Manager's `FIDE_Export` file, which is TRF). Results go through the TORO web system or to fsi@federscacchi.it, "entro 8 giorni dal termine del torneo" (within 8 days of the end). The arbiter sends the results; the organiser sends the forms and the fee.

### Czechia (ŠSČR)

[Classification rules](https://chess.cz/wp-content/uploads/2024/08/Klasifikacni-rad-SSCR_2024_cisty.pdf), in force 1 September 2024 (200), 3.5: the organiser sends the documents to the LOK processor within 7 days. The registration directive, 2.1: "jediným akceptovatelným podkladem je datový soubor ze Swissmanageru" (the only accepted document is a Swiss-Manager data file). FIDE events are registered 14 days ahead with Swiss-Manager's FIDE report.

### Norway (NSF)

The [Norwegian Grand Prix rules 2025](https://dnkjuhc6if10z.cloudfront.net/filarkiv/nsf/Reglementer-og-vedtekter/Norges-Grand-Prix-nytt-reglement-2025-renskrevet.pdf) (200): "Turneringslederen skal senest dagen etter turneringen sende resultatet til NSF på elektronisk format (f.eks Turneringservice-filer eller andre kjente format) til turneringsrapport@sjakk.no" (The director sends the result to NSF by the next day, electronically, for example as Turneringservice files or another known format, to turneringsrapport@sjakk.no.) The general rating rules were not found.

### Finland (SShL)

[Competition rules](https://www.shakkiliitto.fi/wp-content/uploads/2025/06/Suomen-kilpailumaaraykset-1_7_2025.pdf), in force 1 July 2025 (200), section 8: "Kilpailun päätuomari on vastuussa siitä, että raportti kilpailusta ja kilpailun täydelliset tulokset toimitetaan kilpailuraporttilomakkeella SSL:lle." (The chief arbiter makes sure the report and full results reach SShL on the competition report form.) The form itself was not found.

### Sweden (SSF)

Since February 2026 tournaments are reported in a form, "Anmäl ELO-turnering", inside the member system ([SSF news](https://schack.se/nyhet/okategoriserade/2026/02/forenklad-ratingrapportering-och-forandringar-i-ratingorganisationen/), 200): "De gamla formulären som tidigare mejlats in tas helt bort." (The old emailed forms are gone.) The public API that serves the rating list has no upload.

### Lithuania (LŠF)

[FIDE tournament registration](https://www.chessfed.lt/fide-informacija/turnyru-registravimas-fide/) (200): a `.docx` form to the rating administrator, at least 5 days ahead, 30 with norms; €5 a tournament from 1 January 2024, €70 for a late or missing registration. The results file is not stated.

### Canada (CFC)

[Organiser tools](https://chess.ca/en/organizers/tools) (200): "Results are sent to the CFC in two files: myevent.ctr and myevent.tms. These files are input to the Ratings Program. No other format will be accepted." SwissSys, Swiss-Manager and the CFC's Swiss Assistant write them. No spec of either file is published. [Rating fees](https://www.chess.ca/en/organizers/rating-fees/) (200): $3.00 a player plus tax in that format, $5.00 otherwise.

### Japan (JCA)

[What is the rating?](https://japanchess.org/?p=11880) (200): report by email to japanchess.rating@gmail.com, on the JCA's template, with (a) the event's name, date, place, whether it is rapid, and the person in charge; (b) the players with their JCA ID and name in Latin letters; (c) each game: White, result, Black. With Swiss-Manager, the chess-results.com link is enough. Reports in by the 20th count that month.

### Ukraine (UCF)

The federation is at [ukrchess.org.ua](https://ukrchess.org.ua) (200); `ucf.org.ua` is not its site. "Заявки на реєстрацію турнірів та TRF-звіти приймаються виключно електронною поштою." (Registrations and TRF reports are taken by email only.) FIDE reports go to ukrchessrating@gmail.com, national ones to ukrchessrating@ukr.net.

### Uruguay (FUA)

[fuajedrez.org](https://fuajedrez.org/actualizaciones-elo-fua/) (200) publishes a monthly national list. A search summary of the federation's text: "Los reportes de dichos torneos deben enviarse al Rating Officer acompañados del archivo PGN correspondiente." (Reports go to the rating officer with the PGN file.)

### Scotland and Wales

[Chess Scotland](https://www.chessscotland.com/documents/grading/outside.htm) (200) rates its members' games outside Scotland free of charge, standard play only: "Data should be emailed to grading@chessscotland.com". The [Welsh Chess Union](https://www.welshchessunion.uk/Grading/index.php) (200) lists a rating officer for each zone: "Games will not be rated unless the RO is aware of your participation beforehand." Neither names a file format.

## Federations with no written procedure found

These were searched on 2026-11-19 and nothing usable was found on the federation's own site. For a FIDE-rated event the FIDE route above applies, through the federation's FIDE rating officer.

| Country | What was found |
|---|---|
| Germany (DSB) | The DWZ is worked out in DeWIS from what the regional rating officers enter. No file format is published. The DSB's [Wertungsportal API](https://schachde-apps.liga.nu/dsbwertungsportal/apidocs/data.html) (200) only reads (`GET /dwz/tournaments/{uuid}`). FIDE rating officer: elo@schachbund.com ([page](https://www.schachbund.de/fide-rating.html), 200) |
| France (FFE) | "Les demandes d'homologation des Tournois doivent se faire en se connectant à la page de gestion de son compte" (homologation is asked for from the director's account page, [echecs.asso.fr](https://echecs.asso.fr/Default.aspx?Cat=17), 200). The Papi program's file is not documented |
| Greece | An arbiter login on [chessfed.net](https://chessfed.net) (200), under "Υποβολή τουρνουά για αξιολόγηση" (submit tournament for evaluation). No format |
| Israel | [chess.org.il](https://www.chess.org.il/) (200); its rating rules page answers 404 |
| Poland | The [classification rules 2024](https://pzszach.pl/wp-content/uploads/2023/12/Regulamin-Klasyfikacyjny-2024.pdf) (200) say how the national ranking is worked out, not how results are sent |
| Estonia | The rating rules PDF on [maleliit.ee](https://maleliit.ee/) could not be read; old summaries say electronic tables within 30 days |
| Latvia | [sahafederacija.lv](https://sahafederacija.lv/) (200) asks for a player licence; no rating route found |
| Catalonia (FCE) | [escacs.cat](https://escacs.cat/index.php/partides-i-elo/avaluacio-elo) (200) keeps a Catalan Elo for licensed players; how results reach it is not stated |
| Australia (ACF) | The rating pages publish the master files only; the [Ratings By-Law](https://auschess.org.au/wp-content/uploads/2018/09/ACF-Ratings-By-Law.pdf) was not read |
| New Zealand (NZCF) | `newzealandchess.co.nz` refused connections all day. Tornelo says it writes "an NZCF Rating Report" ([Tornelo](https://tornelo.com/supporting-new-zealand-chess-federation-rated-events/), 200) |
| Argentina, Colombia, Costa Rica | The sites load ([FADA](https://www.federacionargentinadeajedrez.org/), [FECODAZ](https://federacioncolombianadeajedrez.com/elo-2/), [FCA](https://fcacostarica.com/procedimiento-elo/)). Colombia and Costa Rica publish a national list; Costa Rica's procedure is in a PDF that was not read |
| Armenia, Georgia, Singapore | The sites load ([chessfed.am](https://www.chessfed.am/en/), [geochess.ge](https://geochess.ge/), [singaporechess.org.sg](https://singaporechess.org.sg/faqs/)) but show no reporting procedure |
| Chile, El Salvador, Mexico, Peru, Venezuela | The federation's site did not answer or did not resolve |
| Bolivia, Cuba, Dominican Republic, Ecuador, Guatemala, Honduras, Iran, Nicaragua, Panama, Puerto Rico, South Korea, Taiwan | No federation site found (`fcf.ir` serves another organisation). Several of these federations put their events on chess-results.com |

## Formats Bragd does not write

| Format | Body | Why not |
|---|---|---|
| ECF rating file | England | Its layout is on LMS pages that answer 403 |
| DWZ evaluation data | Germany | No format published |
| `.ctr` and `.tms` | Canada | No spec published |
| CSV for the Result Submission Center | Switzerland | No spec published |
| Swiss-Manager tournament file | Austria, Czechia, chess-results.com | Proprietary, no spec published |
| Vega `.elo` | Italy | No spec published; Italy also takes TRF |
| Turneringservice files | Norway | No spec published; Norway also takes "other known formats" |
| Papi file | France | No spec published |

Each of these needs the federation to publish its format, or to allow Bragd to write it. Bragd will not upload into a federation's server unless the federation publishes an API and allows it.

## Questions

#### Why does Bragd not send the results for me?

No federation publishes an upload API, and the FIDE rating server is open only to federations' rating officers. The director sends the file, by email or on the federation's site.

#### Is TRF enough for FIDE?

Yes. FIDE's rating server reads TRF. The federation's rating officer may also ask for a form, such as Spain's IT3, or for the PGN.

#### My federation takes a Swiss-Manager file. What can I do?

Ask whether it takes TRF instead. Austria takes TRF from other programs, Italy takes Swiss-Manager's TRF export, and Norway takes "other known formats". Swiss-Manager itself can read a TRF.

#### Why is my federation's row empty?

Its site showed no procedure on 2026-11-19, or did not answer. If you know where your federation documents it, open an issue on Bragd's issue tracker with the address.

---
[← Previous: Rating lists](rating-lists.md) · [Next: Swiss pairing →](swiss-pairing.md)

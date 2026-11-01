[← README](../../README.md) · [Docs index](../README.md) · [Online](README.md)

# pychess.org

pychess.org plays xiangqi, janggi, shogi and many fairy chess Variants. Bragd cannot play there from its own board, and the install wizard links the website instead. This page says why, so nobody has to find out again.

## What pychess.org offers

| What another app needs | pychess.org |
|---|---|
| A sign-in for apps (OAuth) | None. pychess.org signs people in through Lichess, Lishogi, Google or Discord, and keeps a browser session |
| A way to play as a person | None. Seeks, challenges and moves go over its own website's socket, signed in by the browser's session cookie |
| A documented, stable API | Only a bot API: it is for engine accounts, which cannot seek or challenge, and an account that has played a Game cannot become one |
| Terms that allow other apps | Automation only "where the site explicitly supports" it |

Driving the website's socket with a copied cookie would be scraping an undocumented protocol, against its terms. Bragd does not do that.

## What would change it

pychess.org adding a sign-in for apps and an API for a person's Games, as Lichess's Board API is. Then pychess.org gets its own page here, as [Lishogi](lishogi.md) has.

## Questions

#### Can I play xiangqi or janggi online from Bragd?

Not yet. Play them on pychess.org in your browser. Bragd's own server, after these, is meant for the Families no other server lets Bragd play.

#### Can Bragd's Engines play on pychess.org as a bot?

pychess.org's bot API would allow it for a new account made for the purpose, but that is not playing yourself, and Bragd does not offer it.

---
[← Previous: Playing on OGS](ogs.md) · [Next: Reference →](../reference/README.md)

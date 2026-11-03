# Ratings on the Bragd server are Glicko-2, one per Game family, updated after every Game

A rated Game on the Bragd server changes both players' ratings in its Game family as soon as it ends. The rating is Glicko-2, the system Clubs use by default (`crates/bragd-clubs/src/rating.rs`, the same code), started at 1500 with a deviation of 350 and a volatility of 0.06, with τ = 0.5. Each Game is a rating period of its own. A rating whose deviation is above 110 is shown with a `?`: it is still provisional.

Glickman meant a rating period to hold ten to fifteen Games, so that volatility is estimated from many results at once. We update after each Game instead, as Lichess does, because players expect to see their rating move when a Game ends, and a batch run would need a timer on the server and a period during which ratings are stale. The cost is a volatility that reacts to single results more than the paper intends; with τ = 0.5 the change stays small.

One rating per Family, not per Variant or per time control: there are few players on the Bragd server, and splitting them further would leave most ratings provisional. Chess960 and chess share one rating, as do the three draughts rulesets. Elo was the other choice; Glicko-2's deviation says how sure a new player's rating is, which matters most on a small server.

A Game that ends because a player deleted their account is not rated: the account and its ratings are gone.

-- The dice Families on the Bragd server: backgammon, Ur and senet. The referee throws the dice
-- and sticks for both players from the server's own random source, and records each throw as an
-- entry in `public.moves`, as the rules write it. No player ever sends a throw.

-- Who moves first when the rules decide it at the start, as backgammon's opening throw does
-- (`white` or `black`). Set by the referee only.
alter table public.games add column initial text;

-- A challenge names the players and the Variant, nothing more: every other column of a Game is
-- the referee's, so a player cannot start a Game from a position or a seat of their choosing.
revoke insert on public.games from authenticated;
grant insert (first, second, challenger, variant) on public.games to authenticated;

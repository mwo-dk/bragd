-- A challenge names both players. Since `20261103000000_delete_account.sql` a seat may be empty,
-- for a deleted player's ended Games, and `challenger in (first, second)` and
-- `bragd.blocked_between` both pass when one is NULL: a blocked player could leave an offer
-- nobody can answer. Only `bragd.forget` empties a seat, and it runs as the owner.
drop policy "a player challenges another" on public.games;
create policy "a player challenges another" on public.games
  for insert to authenticated with check (challenger = auth.uid() and first is not null and second is not null
    and status = 'offered' and result is null and draw_offer is null
    and not bragd.blocked_between(first, second));

-- Live Games: each change the referee makes to a Game (a challenge, a move, a throw, a clock, a
-- result, a Chat line) is broadcast over Supabase Realtime to the private topic `player:<id>`
-- of each of its two players, so their Bragd asks how things stand at once. The broadcast names
-- only the Game; the Game itself is still read from the game service, which stays the referee.
-- Only the player may listen on their own topic, and nobody sends on it but the database.

create function bragd.may_listen(topic text) returns boolean
  language sql stable security definer set search_path = '' as $$
  select topic = 'player:' || auth.uid()::text
$$;

grant execute on function bragd.may_listen(text) to authenticated;

create policy "a player hears of their own Games" on realtime.messages
  for select to authenticated using (realtime.messages.extension = 'broadcast' and bragd.may_listen(realtime.topic()));

create function bragd.tell_players() returns trigger
  language plpgsql security definer set search_path = '' as $$
declare
  game uuid;
  player uuid;
begin
  -- Each branch reads only its own table's column.
  if tg_table_name = 'games' then
    game := new.id;
  else
    game := new.game_id;
  end if;
  -- A deleted player's side is empty: nobody to tell.
  for player in select p from public.games g, unnest(array[g.first, g.second]) p where g.id = game and p is not null loop
    perform realtime.send(jsonb_build_object('from', 'bragd', 'signal', jsonb_build_object('game', game)), 'signal', 'player:' || player, true);
  end loop;
  return null;
end
$$;

create trigger games_tell_players after insert or update on public.games
  for each row execute function bragd.tell_players();
create trigger moves_tell_players after insert on public.moves
  for each row execute function bragd.tell_players();
create trigger chat_tell_players after insert on public.chat
  for each row execute function bragd.tell_players();

-- Deleting an account also drops what was broadcast to the player's own topic.
create or replace function bragd.forget(who uuid, keep_games boolean) returns void
  language plpgsql security definer set search_path = '' as $$
begin
  if keep_games then
    update public.games set status = 'over', result = case when first = who then '0-1' else '1-0' end,
      reason = 'resign', draw_offer = null, updated_at = now()
      where status = 'playing' and who in (first, second);
    update public.games set
      first = nullif(first, who), second = nullif(second, who),
      challenger = nullif(challenger, who), draw_offer = nullif(draw_offer, who)
      where status = 'over' and who in (first, second);
    -- A Game both of whose players are gone is nobody's.
    delete from public.games where first is null and second is null;
  end if;
  delete from auth.users where id = who;
  -- Call signals name both players in their topic, and a player's own topic names them. Realtime
  -- drops them within days anyway, so a project whose role may not delete them here still
  -- deletes the account.
  begin
    delete from realtime.messages where topic = 'player:' || who::text
      or (topic like 'call:%' and who::text in (split_part(topic, ':', 2), split_part(topic, ':', 3)));
  exception when insufficient_privilege then null;
  end;
end
$$;

revoke all on function bragd.forget(uuid, boolean) from public, anon, authenticated;

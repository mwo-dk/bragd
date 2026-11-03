-- Deleting an account from Bragd (Play with Friends → Delete my account). The game service runs
-- `bragd.forget` for the caller, as the referee: a player cannot delete their own Auth user.
--
-- Everything with the player's id goes: their Auth user, and by the cascades from it their
-- profile, Friends, requests, invites, blocks, reports made and about them, Clubs they made and
-- their memberships, and their Chat lines. Their Games go too, unless they agree to leave the
-- Games that ended to the other players: those keep the moves and the result, against a deleted
-- player, whose side is empty. A Game under way then ends as their resignation first.

alter table public.games
  alter column first drop not null,
  alter column second drop not null,
  alter column challenger drop not null;

create function bragd.forget(who uuid, keep_games boolean) returns void
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
  -- Call signals name both players in their topic. Realtime drops them within days anyway, so
  -- a project whose role may not delete them here still deletes the account.
  begin
    delete from realtime.messages where topic like 'call:%' and who::text in (split_part(topic, ':', 2), split_part(topic, ':', 3));
  exception when insufficient_privilege then null;
  end;
end
$$;

revoke all on function bragd.forget(uuid, boolean) from public, anon, authenticated;

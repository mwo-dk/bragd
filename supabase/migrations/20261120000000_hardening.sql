-- What one player may not do to the others, after the review of 2026-11-20: forge a row through
-- Supabase's own API, write without end, store junk, or take the others' data with them when
-- they delete their account.

-- Supabase grants `anon` and `authenticated` everything on every new table in `public`, so the
-- column grants of the earlier migrations added nothing. Every grant is taken back and restated
-- here, column by column, and tables made later start with none.
alter default privileges in schema public revoke all on tables from anon, authenticated;
alter default privileges in schema public revoke all on sequences from anon, authenticated;
alter default privileges in schema public revoke all on functions from anon, authenticated;
revoke all on all tables in schema public from anon, authenticated;
revoke all on all sequences in schema public from anon, authenticated;

grant select, insert (id, name), update (name, club_friends) on public.profiles to authenticated;
grant select, insert (first, second, challenger, variant, clock_ms, increment_ms, rated) on public.games to authenticated;
grant select on public.moves to authenticated;
grant select, insert (blocker, blocked), delete on public.blocks to authenticated;
grant select, insert (a, b, asked_by), update (accepted), delete on public.friendships to authenticated;
grant select, insert (code, inviter), delete on public.invites to authenticated;
grant select, insert (id, name, code, created_by) on public.clubs to authenticated;
grant select, delete on public.club_members to authenticated;
grant insert (reported, game_id, reason) on public.reports to authenticated;
grant select, insert (game_id, text) on public.chat to authenticated;
grant select on public.ratings to authenticated;
grant select, insert (player, variant, clock_ms, increment_ms, rated), delete on public.seeks to authenticated;

-- Accepting a request changes nothing but `accepted`: the row must still be the caller's, asked
-- by the other one, after the update as before it.
drop policy "the one asked accepts" on public.friendships;
create policy "the one asked accepts" on public.friendships
  for update to authenticated using (auth.uid() in (a, b) and asked_by <> auth.uid())
  with check (auth.uid() in (a, b) and asked_by <> auth.uid() and accepted and not bragd.blocked_between(a, b));

-- A Variant is a catalogue key, never free text.
alter table public.games add constraint games_variant check (variant ~ '^[a-z0-9_-]{1,40}$');
alter table public.seeks add constraint seeks_variant check (variant ~ '^[a-z0-9_-]{1,40}$');

-- How much one player may write. The trigger's arguments: the column naming the player, the
-- column of when, how far back rows count, how many may be there, and which rows count. A lock
-- for each player and table makes two writes at once count each other.
create function bragd.quota() returns trigger
  language plpgsql security definer set search_path = '' as $$
declare
  owner uuid := (to_jsonb(new) ->> tg_argv[0])::uuid;
  n bigint;
begin
  if owner is null then
    return new;
  end if;
  perform pg_advisory_xact_lock(hashtextextended(tg_table_name || ':' || owner::text, 0));
  execute format('select count(*) from %I.%I where %I = $1 and %I > now() - $2::interval and (%s)',
    tg_table_schema, tg_table_name, tg_argv[0], tg_argv[1], tg_argv[4])
    into n using owner, tg_argv[2];
  if n >= tg_argv[3]::int then
    raise exception 'too many rows in % for one player', tg_table_name using errcode = 'PT429';
  end if;
  return new;
end
$$;

create index reports_reporter on public.reports (reporter, created_at);
create index invites_inviter on public.invites (inviter, created_at);
create index clubs_created_by on public.clubs (created_by, created_at);
create index games_challenger on public.games (challenger, created_at);
create index friendships_asked_by on public.friendships (asked_by, created_at);
create index chat_sender on public.chat (sender, said_at);

create trigger quota before insert on public.reports
  for each row execute function bragd.quota('reporter', 'created_at', '1 day', '20', 'true');
-- An invite lasts 7 days.
create trigger quota before insert on public.invites
  for each row execute function bragd.quota('inviter', 'created_at', '7 days', '20', 'true');
create trigger quota before insert on public.clubs
  for each row execute function bragd.quota('created_by', 'created_at', '1 day', '5', 'true');
-- A challenge stands for 7 days.
create trigger quota_open before insert on public.games
  for each row execute function bragd.quota('challenger', 'created_at', '7 days', '10', $$status = 'offered'$$);
create trigger quota before insert on public.games
  for each row execute function bragd.quota('challenger', 'created_at', '1 day', '200', 'true');
create trigger quota before insert on public.friendships
  for each row execute function bragd.quota('asked_by', 'created_at', '1 day', '50', 'true');
create trigger quota before insert on public.chat
  for each row execute function bragd.quota('sender', 'said_at', '1 minute', '30', 'true');

-- Deleting an account no longer takes the others' data with it. A report about the player
-- stays with the operator, with the name they had and without their id. A Chat line both
-- players kept stays with its Game, from a deleted player. A Club passes to another member,
-- and goes only when it has none.
alter table public.reports
  add column reported_name text,
  alter column reported drop not null,
  drop constraint reports_reported_fkey,
  add constraint reports_reported_fkey foreign key (reported) references public.profiles (id) on delete set null;
create index reports_reported on public.reports (reported);

alter table public.chat
  alter column sender drop not null,
  drop constraint chat_sender_fkey,
  add constraint chat_sender_fkey foreign key (sender) references public.profiles (id) on delete set null;

create function bragd.leave_behind() returns trigger
  language plpgsql security definer set search_path = '' as $$
begin
  update public.reports set reported_name = old.name where reported = old.id;
  update public.clubs c set created_by = m.member
    from (select distinct on (club_id) club_id, member from public.club_members where member <> old.id order by club_id, member) m
    where c.created_by = old.id and m.club_id = c.id;
  return old;
end
$$;

create trigger leave_behind before delete on public.profiles
  for each row execute function bragd.leave_behind();

revoke all on function bragd.quota(), bragd.leave_behind() from public, anon, authenticated;

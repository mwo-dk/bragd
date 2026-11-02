-- The Bragd server's first tables: public profiles and Games between Bragd players.
--
-- Supabase owns `auth.users` and `auth.uid()`; the tests stand those in on plain Postgres
-- (crates/bragd-server/tests/supabase-stub.sql). Row-level security is the rule for every user:
-- the game service runs each request as the role `authenticated` with the caller's JWT claims,
-- as PostgREST does, and only the referee's own writes (moves, a Game's status) bypass it.

-- A player's public face: the name others find them by.
create table public.profiles (
  id uuid primary key references auth.users (id) on delete cascade,
  name text not null unique check (char_length(name) between 2 and 30 and name !~ '[[:cntrl:]]' and name = btrim(name)),
  created_at timestamptz not null default now()
);

alter table public.profiles enable row level security;

create policy "profiles are read by players" on public.profiles
  for select to authenticated using (true);
create policy "a player makes their own profile" on public.profiles
  for insert to authenticated with check (id = auth.uid());
create policy "a player renames their own profile" on public.profiles
  for update to authenticated using (id = auth.uid()) with check (id = auth.uid());

-- A Game between two Bragd players. `first` plays the Side that moves first. It starts as a
-- challenge (`offered`), which the other player accepts or declines, or the challenger cancels.
create table public.games (
  id uuid primary key default gen_random_uuid(),
  first uuid not null references public.profiles (id) on delete cascade,
  second uuid not null references public.profiles (id) on delete cascade,
  challenger uuid not null,
  variant text not null,
  status text not null default 'offered' check (status in ('offered', 'declined', 'cancelled', 'playing', 'over')),
  -- `1-0`, `0-1`, `½-½` or a Family's own score, once over; `reason` is why.
  result text,
  reason text,
  -- The player whose draw offer stands.
  draw_offer uuid,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  check (first <> second),
  check (challenger in (first, second))
);

create index games_first on public.games (first);
create index games_second on public.games (second);

alter table public.games enable row level security;

create policy "a Game is seen by its two players" on public.games
  for select to authenticated using (auth.uid() in (first, second));
-- Everything after the challenge is the referee's: accepting, moves, the result.
create policy "a player challenges another" on public.games
  for insert to authenticated with check (challenger = auth.uid() and status = 'offered' and result is null and draw_offer is null);

-- The moves, as Bragd's rules write them. Only the referee writes here, after the rules found
-- the move legal and it was the mover's turn.
create table public.moves (
  game_id uuid not null references public.games (id) on delete cascade,
  ply integer not null check (ply >= 0),
  entry text not null,
  played_at timestamptz not null default now(),
  primary key (game_id, ply)
);

alter table public.moves enable row level security;

create policy "moves are seen by the Game's players" on public.moves
  for select to authenticated using (exists (select 1 from public.games g where g.id = game_id and auth.uid() in (g.first, g.second)));

grant usage on schema public to authenticated;
grant select, insert, update (name) on public.profiles to authenticated;
grant select, insert on public.games to authenticated;
grant select on public.moves to authenticated;
revoke all on public.profiles, public.games, public.moves from anon;

-- Clocks, ratings and seeks for Games on the Bragd server.
--
-- A clock is kept by the referee alone, on the database's own time: the time a player used is
-- `now()` less `turn_started`, charged when they move, and their flag falls on the server when
-- it runs out. A rating is Glicko-2, one for each player in each Game family
-- (docs/adr/0007-ratings-on-the-bragd-server.md), and only the referee writes it. A seek is an
-- open challenge that any player may take, unless one of the two blocks the other.

-- A Game's time control: `clock_ms` for each player, and `increment_ms` added after each of
-- their turns; no clock when `clock_ms` is null. `left_first` and `left_second` are each
-- player's time as it stood when `turn_started`, and `clock_side` is the Side whose clock runs
-- (0 the first, 1 the second).
alter table public.games
  add column clock_ms integer check (clock_ms between 60000 and 10800000),
  add column increment_ms integer not null default 0 check (increment_ms between 0 and 180000),
  add column rated boolean not null default false,
  add column left_first integer,
  add column left_second integer,
  add column clock_side smallint check (clock_side in (0, 1)),
  add column turn_started timestamptz;

-- The challenger chooses the time control and whether the Game is rated; the clock's state is
-- the referee's.
grant insert (clock_ms, increment_ms, rated) on public.games to authenticated;

-- A player's rating in one Game family.
create table public.ratings (
  player uuid not null references public.profiles (id) on delete cascade,
  family text not null,
  rating double precision not null default 1500,
  deviation double precision not null default 350,
  volatility double precision not null default 0.06,
  games integer not null default 0,
  primary key (player, family)
);

alter table public.ratings enable row level security;

create policy "ratings are read by players who do not block each other" on public.ratings
  for select to authenticated using (player = auth.uid() or not bragd.blocked_between(player, auth.uid()));

-- A seek: a Game offered to any player. One at a time for each player.
create table public.seeks (
  id uuid primary key default gen_random_uuid(),
  player uuid not null unique references public.profiles (id) on delete cascade,
  variant text not null,
  clock_ms integer check (clock_ms between 60000 and 10800000),
  increment_ms integer not null default 0 check (increment_ms between 0 and 180000),
  rated boolean not null default false,
  created_at timestamptz not null default now()
);

alter table public.seeks enable row level security;

create policy "seeks are read by players who do not block each other" on public.seeks
  for select to authenticated using (player = auth.uid() or not bragd.blocked_between(player, auth.uid()));
create policy "a player seeks" on public.seeks
  for insert to authenticated with check (player = auth.uid());
create policy "a player takes back their seek" on public.seeks
  for delete to authenticated using (player = auth.uid());

grant select on public.ratings to authenticated;
grant select, delete on public.seeks to authenticated;
grant insert (player, variant, clock_ms, increment_ms, rated) on public.seeks to authenticated;
revoke all on public.ratings, public.seeks from anon;

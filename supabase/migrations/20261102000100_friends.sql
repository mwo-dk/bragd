-- Friends, blocks, reports, and Clubs on the Bragd server.
--
-- A Friend is another player you both accepted: by a request one makes and the other accepts,
-- or by an invite code one hands the other. Members of a Club you share count as Friends while
-- both of you allow it (`profiles.club_friends`). A block ends a friendship, refuses the
-- blocked player's requests and challenges, and hides each from the other. A report goes to the
-- server's operator, and no player can read it back.

-- The rules' helpers live outside `public`, which Supabase's API exposes: a player can use them
-- through the policies, but cannot ask them about anyone they like.
create schema bragd;
grant usage on schema bragd to authenticated;

alter table public.profiles add column club_friends boolean not null default false;
grant update (club_friends) on public.profiles to authenticated;

create table public.blocks (
  blocker uuid not null references public.profiles (id) on delete cascade,
  blocked uuid not null references public.profiles (id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (blocker, blocked),
  check (blocker <> blocked)
);

-- Whether either of `a` and `b` blocks the other. Security definer: a player cannot read who
-- blocks them, but the rules must.
create function bragd.blocked_between(p uuid, q uuid) returns boolean
  language sql stable security definer set search_path = '' as $$
  select exists (select 1 from public.blocks k where (k.blocker = p and k.blocked = q) or (k.blocker = q and k.blocked = p))
$$;

-- A friendship, stored once per pair: `a` is the smaller id. Pending until the one who did not
-- ask accepts.
create table public.friendships (
  a uuid not null references public.profiles (id) on delete cascade,
  b uuid not null references public.profiles (id) on delete cascade,
  asked_by uuid not null,
  accepted boolean not null default false,
  created_at timestamptz not null default now(),
  primary key (a, b),
  check (a < b),
  check (asked_by in (a, b))
);

create table public.invites (
  code text primary key check (char_length(code) between 8 and 64),
  inviter uuid not null references public.profiles (id) on delete cascade,
  created_at timestamptz not null default now()
);

create table public.clubs (
  id uuid primary key default gen_random_uuid(),
  name text not null check (char_length(name) between 1 and 80),
  code text not null unique check (char_length(code) between 8 and 64),
  created_by uuid not null references public.profiles (id) on delete cascade,
  created_at timestamptz not null default now()
);

create table public.club_members (
  club_id uuid not null references public.clubs (id) on delete cascade,
  member uuid not null references public.profiles (id) on delete cascade,
  primary key (club_id, member)
);

-- Whether `a` and `b` are Friends: accepted, or in one Club with both allowing it, and neither
-- blocking the other.
create function bragd.are_friends(p uuid, q uuid) returns boolean
  language sql stable security definer set search_path = '' as $$
  select p <> q and not bragd.blocked_between(p, q) and (
    exists (select 1 from public.friendships f where f.a = least(p, q) and f.b = greatest(p, q) and f.accepted)
    or exists (
      select 1 from public.club_members mp join public.club_members mq on mp.club_id = mq.club_id
      join public.profiles pp on pp.id = mp.member join public.profiles pq on pq.id = mq.member
      where mp.member = p and mq.member = q and pp.club_friends and pq.club_friends
    )
  )
$$;

-- Whether the caller is a member of `club`. Security definer, so the policy on the members
-- does not read itself.
create function bragd.member_of(club uuid) returns boolean
  language sql stable security definer set search_path = '' as $$
  select exists (select 1 from public.club_members m where m.club_id = club and m.member = auth.uid())
$$;

create table public.reports (
  id bigint generated always as identity primary key,
  reporter uuid not null default auth.uid() references public.profiles (id) on delete cascade,
  reported uuid not null references public.profiles (id) on delete cascade,
  game_id uuid references public.games (id) on delete set null,
  reason text not null check (char_length(reason) between 1 and 2000),
  created_at timestamptz not null default now(),
  check (reporter <> reported)
);

alter table public.blocks enable row level security;
alter table public.friendships enable row level security;
alter table public.invites enable row level security;
alter table public.clubs enable row level security;
alter table public.club_members enable row level security;
alter table public.reports enable row level security;

create policy "a player sees their own blocks" on public.blocks
  for select to authenticated using (blocker = auth.uid());
create policy "a player blocks" on public.blocks
  for insert to authenticated with check (blocker = auth.uid());
create policy "a player unblocks" on public.blocks
  for delete to authenticated using (blocker = auth.uid());

create policy "a friendship is seen by its two" on public.friendships
  for select to authenticated using (auth.uid() in (a, b));
create policy "a player asks to be Friends" on public.friendships
  for insert to authenticated with check (asked_by = auth.uid() and auth.uid() in (a, b) and not accepted and not bragd.blocked_between(a, b));
create policy "the one asked accepts" on public.friendships
  for update to authenticated using (auth.uid() in (a, b) and asked_by <> auth.uid()) with check (accepted and not bragd.blocked_between(a, b));
create policy "either ends it" on public.friendships
  for delete to authenticated using (auth.uid() in (a, b));

create policy "an inviter sees their invites" on public.invites
  for select to authenticated using (inviter = auth.uid());
create policy "a player invites" on public.invites
  for insert to authenticated with check (inviter = auth.uid());
create policy "an inviter takes an invite back" on public.invites
  for delete to authenticated using (inviter = auth.uid());

create policy "a Club is seen by its members" on public.clubs
  for select to authenticated using (bragd.member_of(id));
create policy "a player makes a Club" on public.clubs
  for insert to authenticated with check (created_by = auth.uid());
create policy "members see each other" on public.club_members
  for select to authenticated using (bragd.member_of(club_id));
create policy "a member leaves" on public.club_members
  for delete to authenticated using (member = auth.uid());

create policy "a player reports" on public.reports
  for insert to authenticated with check (reporter = auth.uid());

-- Blocks hide players from each other, and a blocked player cannot challenge.
drop policy "profiles are read by players" on public.profiles;
create policy "profiles are read by players who do not block each other" on public.profiles
  for select to authenticated using (id = auth.uid() or not bragd.blocked_between(id, auth.uid()));
drop policy "a player challenges another" on public.games;
create policy "a player challenges another" on public.games
  for insert to authenticated with check (challenger = auth.uid() and status = 'offered' and result is null and draw_offer is null
    and not bragd.blocked_between(first, second));

grant select, insert, delete on public.blocks to authenticated;
grant select, insert, update (accepted), delete on public.friendships to authenticated;
grant select, insert, delete on public.invites to authenticated;
grant select, insert on public.clubs to authenticated;
grant select, delete on public.club_members to authenticated;
grant insert on public.reports to authenticated;
revoke all on all functions in schema bragd from public;
grant execute on all functions in schema bragd to authenticated;
revoke all on public.blocks, public.friendships, public.invites, public.clubs, public.club_members, public.reports from anon;

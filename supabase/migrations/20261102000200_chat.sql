-- Chat in Games between Bragd players. It opens only while both players say yes; either closes
-- it for both. Its lines are kept with the Game only when both players ticked *Keep*: when the
-- Game ends, or is declined or cancelled, they are deleted otherwise.

alter table public.games
  add column chat_first boolean not null default false,
  add column chat_second boolean not null default false,
  add column keep_first boolean not null default false,
  add column keep_second boolean not null default false;

create table public.chat (
  id bigint generated always as identity primary key,
  game_id uuid not null references public.games (id) on delete cascade,
  sender uuid not null default auth.uid() references public.profiles (id) on delete cascade,
  text text not null check (char_length(text) between 1 and 500),
  said_at timestamptz not null default now()
);

create index chat_game on public.chat (game_id, id);

alter table public.chat enable row level security;

create policy "a Game's Chat is read by its players" on public.chat
  for select to authenticated using (exists (select 1 from public.games g where g.id = game_id and auth.uid() in (g.first, g.second)));
create policy "a player says a line while both agree" on public.chat
  for insert to authenticated with check (
    sender = auth.uid()
    and exists (select 1 from public.games g where g.id = game_id and auth.uid() in (g.first, g.second)
      and g.status = 'playing' and g.chat_first and g.chat_second)
  );

grant select, insert on public.chat to authenticated;
revoke all on public.chat from anon;

-- A Game that ends keeps its Chat only when both players said so.
create function bragd.forget_chat() returns trigger
  language plpgsql security definer set search_path = '' as $$
begin
  if new.status in ('over', 'declined', 'cancelled') and not (new.keep_first and new.keep_second) then
    delete from public.chat where game_id = new.id;
  end if;
  return new;
end
$$;

create trigger forget_chat after update of status on public.games
  for each row when (old.status is distinct from new.status) execute function bragd.forget_chat();

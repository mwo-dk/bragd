-- What Supabase provides that the migrations rely on, for plain Postgres in tests: the roles,
-- `auth.users`, and `auth.uid()` read from the request's JWT claims as Supabase reads them.
create role anon nologin;
create role authenticated nologin;
create role service_role nologin bypassrls;

create schema auth;
grant usage on schema auth to anon, authenticated, service_role;

create table auth.users (
  id uuid primary key,
  email text
);

create function auth.uid() returns uuid language sql stable as $$
  select nullif(nullif(current_setting('request.jwt.claims', true), '')::jsonb ->> 'sub', '')::uuid
$$;

-- Realtime's table of broadcast messages, whose policies decide who joins a private topic, and
-- `realtime.topic()`, the topic being joined, as Realtime sets it.
create schema realtime;
grant usage on schema realtime to anon, authenticated, service_role;

create table realtime.messages (
  id bigint generated always as identity primary key,
  topic text not null,
  extension text not null,
  payload jsonb,
  event text,
  private boolean default true,
  inserted_at timestamptz not null default now()
);
alter table realtime.messages enable row level security;
grant select, insert on realtime.messages to authenticated;

create function realtime.topic() returns text language sql stable as $$
  select nullif(current_setting('realtime.topic', true), '')
$$;

-- Realtime's broadcast from the database: a row in `realtime.messages`, which Realtime sends to
-- whoever the topic's policies let listen.
create function realtime.send(payload jsonb, event text, topic text, private boolean default true) returns void
  language sql as $$
  insert into realtime.messages (topic, extension, payload, event, private) values (topic, 'broadcast', payload, event, private)
$$;

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

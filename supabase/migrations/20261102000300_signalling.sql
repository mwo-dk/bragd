-- Call signalling over Supabase Realtime: a private broadcast topic `call:<a>:<b>` for each pair
-- of players, `a` the smaller id. Only those two may join it, and only while they are Friends:
-- a Call is only ever between Friends. Media never passes here, only offers, answers and ICE.

create function bragd.may_signal(topic text) returns boolean
  language sql stable security definer set search_path = '' as $$
  select topic ~ '^call:[0-9a-f-]{36}:[0-9a-f-]{36}$'
    and split_part(topic, ':', 2) < split_part(topic, ':', 3)
    and auth.uid() in (split_part(topic, ':', 2)::uuid, split_part(topic, ':', 3)::uuid)
    and bragd.are_friends(split_part(topic, ':', 2)::uuid, split_part(topic, ':', 3)::uuid)
$$;

grant execute on function bragd.may_signal(text) to authenticated;

create policy "Friends receive each other's Call signals" on realtime.messages
  for select to authenticated using (realtime.messages.extension = 'broadcast' and bragd.may_signal(realtime.topic()));
create policy "Friends send each other Call signals" on realtime.messages
  for insert to authenticated with check (realtime.messages.extension = 'broadcast' and bragd.may_signal(realtime.topic()));

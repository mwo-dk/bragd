-- Lookups the game service makes on every `/v1/state`, after the review of 2026-11-20: a
-- player's Clubs by member (`bragd.are_friends`, the Friends list), and the friendships where the
-- player is the second of the pair (the primary key starts with the first).
create index club_members_member on public.club_members (member);
create index friendships_b on public.friendships (b);

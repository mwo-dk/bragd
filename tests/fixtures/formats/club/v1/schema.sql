CREATE TABLE club (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE leagues (id INTEGER PRIMARY KEY, data TEXT NOT NULL);
CREATE TABLE log (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    who TEXT NOT NULL,
    member TEXT NOT NULL,
    what TEXT NOT NULL,
    old TEXT NOT NULL,
    new TEXT NOT NULL,
    why TEXT NOT NULL
);
CREATE TABLE members (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    fide_id TEXT NOT NULL DEFAULT '',
    national_id TEXT NOT NULL DEFAULT '',
    federation TEXT NOT NULL DEFAULT '',
    lichess TEXT NOT NULL DEFAULT '',
    rating REAL NOT NULL,
    deviation REAL NOT NULL,
    volatility REAL NOT NULL
);
CREATE TABLE tournaments (id INTEGER PRIMARY KEY, data TEXT NOT NULL);

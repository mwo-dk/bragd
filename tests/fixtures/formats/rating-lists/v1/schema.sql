CREATE TABLE lists (list TEXT PRIMARY KEY, players INTEGER NOT NULL, at INTEGER NOT NULL);
CREATE VIRTUAL TABLE names USING fts5 (key, content = 'players', content_rowid = 'rowid', tokenize = 'unicode61', prefix = '2 3');
CREATE TABLE players (
    list TEXT NOT NULL,
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    key TEXT NOT NULL,
    federation TEXT NOT NULL,
    club TEXT NOT NULL,
    title TEXT NOT NULL,
    rating INTEGER,
    rapid INTEGER,
    blitz INTEGER,
    born INTEGER,
    fide_id TEXT NOT NULL,
    PRIMARY KEY (list, id)
);
CREATE INDEX players_fide ON players (fide_id);
CREATE INDEX players_id ON players (id);

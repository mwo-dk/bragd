CREATE TABLE game_sources (
    game_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    -- The headers as this Source has them, for merging them again.
    headers TEXT NOT NULL,
    -- The main line's Comments and NAGs as this Source has them (`Note`s as JSON); NULL for none.
    notes TEXT,
    PRIMARY KEY (game_id, source, source_ref)
);
CREATE TABLE games (
    id INTEGER PRIMARY KEY,
    family TEXT NOT NULL,
    variant TEXT NOT NULL,
    -- Every header, merged from the Sources, as a JSON object.
    headers TEXT NOT NULL,
    -- The headers Slices filter on, from `headers`.
    white TEXT NOT NULL DEFAULT '',
    black TEXT NOT NULL DEFAULT '',
    white_elo INTEGER,
    black_elo INTEGER,
    date TEXT NOT NULL DEFAULT '',
    event TEXT NOT NULL DEFAULT '',
    site TEXT NOT NULL DEFAULT '',
    eco TEXT NOT NULL DEFAULT '',
    opening TEXT NOT NULL DEFAULT '',
    result TEXT NOT NULL,
    -- The main line, moves apart by spaces; chess in SAN as Bragd writes it.
    moves TEXT NOT NULL,
    -- The start position; NULL for the Variant's usual one.
    start TEXT,
    ply_count INTEGER NOT NULL,
    zobrist_final INTEGER NOT NULL,
    zobrist_chain_hash INTEGER NOT NULL
);
CREATE TABLE near_duplicates (
    short_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    long_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    PRIMARY KEY (short_id, long_id)
);
CREATE TABLE player_names (name TEXT PRIMARY KEY, games INTEGER NOT NULL);
CREATE TABLE player_names_fresh (fresh INTEGER NOT NULL);
CREATE VIRTUAL TABLE player_words USING fts5 (name, content = 'player_names', tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3');
CREATE TABLE positions (
    hash INTEGER NOT NULL,
    game_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    ply INTEGER NOT NULL
);
CREATE TABLE slices (
    name TEXT PRIMARY KEY,
    query TEXT NOT NULL,
    materialised INTEGER NOT NULL DEFAULT 0,
    path TEXT
);
CREATE INDEX games_black ON games (black);
CREATE INDEX games_black_elo ON games (black_elo);
CREATE INDEX games_chain ON games (zobrist_chain_hash);
CREATE INDEX games_date ON games (date);
CREATE INDEX games_eco ON games (eco);
CREATE INDEX games_white ON games (white);
CREATE INDEX games_white_elo ON games (white_elo);
CREATE INDEX near_duplicates_long ON near_duplicates (long_id);
CREATE INDEX positions_game ON positions (game_id);
CREATE INDEX positions_hash ON positions (hash);
CREATE TRIGGER games_names_added AFTER INSERT ON games BEGIN DELETE FROM player_names_fresh; END;
CREATE TRIGGER games_names_changed AFTER UPDATE OF white, black ON games BEGIN DELETE FROM player_names_fresh; END;
CREATE TRIGGER games_names_deleted AFTER DELETE ON games BEGIN DELETE FROM player_names_fresh; END;

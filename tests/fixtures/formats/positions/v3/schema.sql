CREATE TABLE annotations (
    family TEXT NOT NULL,
    variant TEXT NOT NULL,
    -- The position the move was played in.
    key INTEGER NOT NULL,
    move TEXT NOT NULL,
    edition INTEGER NOT NULL REFERENCES editions (n),
    -- `Mark::sign`, or empty.
    mark TEXT NOT NULL,
    better TEXT NOT NULL,
    -- `Motif::id`s, separated by spaces.
    motifs TEXT NOT NULL,
    depth INTEGER NOT NULL,
    time INTEGER NOT NULL,
    seq INTEGER NOT NULL,
    PRIMARY KEY (family, variant, key, move, edition)
) WITHOUT ROWID;
CREATE TABLE changes (n INTEGER NOT NULL);
CREATE TABLE editions (
    n INTEGER PRIMARY KEY,
    -- `Edition::id`, the same on every machine; `n` is this store's own.
    id TEXT NOT NULL UNIQUE,
    engine TEXT NOT NULL,
    version TEXT NOT NULL,
    network TEXT,
    image TEXT
);
CREATE TABLE evaluations (
    family TEXT NOT NULL,
    variant TEXT NOT NULL,
    key INTEGER NOT NULL,
    edition INTEGER NOT NULL REFERENCES editions (n),
    kind TEXT NOT NULL CHECK (kind IN ('cp', 'mate', 'winrate', 'equity')),
    score REAL NOT NULL,
    lead REAL NOT NULL,
    depth INTEGER NOT NULL,
    line TEXT NOT NULL,
    time INTEGER NOT NULL,
    kind2 TEXT NOT NULL DEFAULT '' CHECK (kind2 IN ('', 'cp', 'mate', 'winrate', 'equity')),
    score2 REAL NOT NULL DEFAULT 0,
    lead2 REAL NOT NULL DEFAULT 0,
    line2 TEXT NOT NULL DEFAULT '',
    -- The change number (`sync`): the store's counter when the row last changed.
    seq INTEGER NOT NULL,
    PRIMARY KEY (family, variant, key, edition)
) WITHOUT ROWID;
CREATE TABLE pulled (
    peer TEXT PRIMARY KEY,
    store TEXT NOT NULL,
    seq INTEGER NOT NULL,
    time INTEGER NOT NULL,
    rows INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE store (id TEXT NOT NULL);
CREATE INDEX annotations_seq ON annotations (seq);
CREATE INDEX evaluations_seq ON evaluations (seq);

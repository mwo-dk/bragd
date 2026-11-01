CREATE TABLE translations (
    original TEXT NOT NULL,
    lang TEXT NOT NULL,
    text TEXT NOT NULL,
    model TEXT NOT NULL,
    time INTEGER NOT NULL,
    PRIMARY KEY (original, lang)
) WITHOUT ROWID;

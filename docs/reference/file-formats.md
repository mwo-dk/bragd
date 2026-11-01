[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# File formats

From 1.0, every file Bragd writes keeps working in later versions. Each file has a format with a version. A later Bragd brings an older file up to date when it opens it. An older Bragd never opens or changes a file from a newer one: it says so, and leaves the file as it is.

## Contents

- [Every file and its version](#every-file-and-its-version)
- [A file from a newer Bragd](#a-file-from-a-newer-bragd)
- [Changing a format](#changing-a-format)
- [Questions](#questions)

## Every file and its version

Where each file lives is in [Files on disk](files.md).

| File | Format | Version kept in | Version at 1.0 | Its migrations |
|---|---|---|---|---|
| `config.toml` | TOML | the top-level key `format` | 1 | `bragd_core::config::FORMAT` |
| `engine-options.toml` | TOML | the top-level key `format` | 1 | `bragd_engines::options::FORMAT` |
| `state.json` | JSON | the key `format` | 1 | `bragd_core::state::FORMAT` |
| A Library (`.bragd`) | SQLite | `PRAGMA user_version` | 1 | `bragd_library::library::FORMAT` |
| A Club file (`.bragd-club`), its `.lock` file, and the Tournaments and Leagues it keeps as JSON | SQLite | `PRAGMA user_version` | 1 | `bragd_clubs::club::FORMAT` |
| `positions.sqlite`, the [Position store](../knowledge/position-store.md) | SQLite | `PRAGMA user_version` | 3 | `bragd_positions::FORMAT` |
| A Position store export (`.parquet`) | Parquet | the file's metadata key `bragd.format` | 1 | `bragd_positions::parquet::FORMAT` and `from_parquet` |
| `rating-lists.sqlite` | SQLite | `PRAGMA user_version` | 1 | `bragd_clubs::lists::FORMAT` |
| `translations.sqlite` | SQLite | `PRAGMA user_version` | 1 | `bragd_model::store::FORMAT` |
| `tournament.json`, in a Tournament bundle | JSON | the key `format` | 1 | `bragd_clubs::export::FORMAT` |
| The Helper's `state.json` | JSON | the key `format` | 1 | `bragd_helper::jobs::FORMAT` |
| `peers.json` | JSON | a list is version 1; a later version is an object with `format` | 1 | `bragd_peers::node::FORMAT` |
| `key.pk8` | PKCS #8 | the standard's own | — | none: a standard format |

A TOML or JSON file without `format` is version 1, so a `config.toml` you write by hand needs no version. Bragd adds `format = 1` at the top the first time it saves a setting.

These files are not versioned, because no Bragd reads them back across versions:

| File | Why |
|---|---|
| `helper.lock`, `helper.sock`, `helper.addr` | Made again each time the Helper starts. The Helper and a window check each other's version when they connect |
| `helper.log` | Written for you to read, never read by Bragd |
| Exports: PGN, TRF, HTML, a Library's Parquet | Standard formats for other programs. Bragd does not read them back as its own |
| Downloads: Sources, tablebases, networks | Other people's formats, kept as they came |

## A file from a newer Bragd

| File | What happens |
|---|---|
| `config.toml` | Bragd starts with the defaults and a notice says why. Settings are not saved, so the file is not changed |
| `state.json` | Bragd starts with the defaults and never saves over the file |
| A Library, a Club file, the Position store, the rating lists, the translations | It is not opened. An error says a newer Bragd wrote it |
| `engine-options.toml` | Engines run with their own defaults. Setting an option is refused |
| The Helper's `state.json` | The Helper starts with no jobs and never saves over the file |
| `peers.json` | Peers are not started |
| A Tournament bundle, a Position store export | The import is refused |

A file whose version is older than 1.0, or not a number, is refused the same way.

## Changing a format

This is for contributors. A change to a stored format bumps its version, adds a migration and a fixture:

1. Add the migration to the end of the format's list in its `FORMAT`. For SQLite it is SQL. For JSON and TOML it is a function that changes the parsed file. Entry *i* takes a file from the format's first version plus *i* to the next. The version is the first version plus the number of migrations, so adding one bumps it.
2. Run the crate's `formats` test with `BRAGD_WRITE_FIXTURES` set to 1. It writes the new version's fixture under `tests/fixtures/formats/<format>/v<N>/`. Commit it, and keep every older one.
3. Run the test again without the variable. It opens every older fixture with today's code and checks its data. That proves the migration.

Each version's folder holds the format's snapshot: `schema.sql` (the tables, as SQLite keeps them) for SQLite, `shape.txt` (every key and its type) for JSON and TOML, and `schema.txt` for Parquet. An SQLite fixture's rows are in `data.sql`. A test fails when today's code writes a schema or shape that differs from the snapshot of the current version. So a format cannot change without a new version. A snapshot already committed is never written over.

Until 1.0.0 is released, the 1.0 baselines can still change, since no user has a file in them. Such a change deletes the baseline's snapshot and writes it again with `BRAGD_WRITE_FIXTURES` set, instead of adding a version.

## Questions

#### Why does Bragd not open a file from a newer version?

An older Bragd cannot know what a newer one changed. Reading the file could misread it, and saving it could lose what the newer version added. Leaving it alone keeps it whole for the newer Bragd.

#### Can I go back to an older Bragd?

Yes, but files the newer version changed stay closed to the older one. Keep a copy of your data folder before you go back, or install the newer version again.

#### Why is the Position store at version 3?

It counted its versions before 1.0, and they are kept, so a store made before 1.0 opens without being made again. Versions 1 and 2 were never released and are not read.

---
[← Previous: Files on disk](files.md) · [Next: Privacy →](privacy.md)

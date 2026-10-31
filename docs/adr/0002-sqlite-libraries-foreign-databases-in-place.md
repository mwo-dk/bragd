# SQLite Libraries; Foreign databases read in place; Parquet only for export

Bragd keeps Games in two layers. A Foreign database (PGN, SCID, ChessBase, SGF, KIF, PDN) is read where it lies and indexed in a cache. Bragd rewrites it only when the user explicitly saves, and is discouraged from doing so for downloaded files. A Library is Bragd's own SQLite file. Imports from Sources land in a Library and are deduplicated there, with Duplicates identified by their Zobrist key chain.

We chose SQLite over DuckDB with Parquet as the working store. A Library is edited all the time: games are merged, headers fixed, annotations added and Slices kept as separate copies. SQLite is transactional and good at small writes, and coxswain's position index already runs on it. Parquet with DuckDB is better for scanning tens of millions of games for statistics, so a Library can be exported to Parquet and DuckDB reads that export. DuckDB never owns the data.

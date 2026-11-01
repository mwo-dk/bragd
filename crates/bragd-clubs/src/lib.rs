//! Clubs: a Club is one SQLite file with its members, their roles and IDs, the club rating and
//! the audit log of every rating a director changed (`club`). Ratings are Glicko-2 or Elo
//! (`rating`). Rating lists, such as FIDE's, are downloaded by the Helper and kept in one file
//! per machine, where check-in finds players by name or ID (`lists`).

pub mod club;
pub mod lists;
pub mod rating;

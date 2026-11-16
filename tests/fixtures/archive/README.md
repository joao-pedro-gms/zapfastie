//! Fixtures for the archive tests. `archive/from-sqlcipher-4.14.db` was written
//! with the library as it stood before the SQLCipher 4.20 port: rusqlite 0.40.2
//! on SQLCipher 4.14.0 and SQLite 3.51.3, keyed with 32 bytes of `0x5a`, with
//! `user_version = 9`, `application_id = 42` and one row in `secrets`. Its
//! `archive_sqlcipher_4_14_opens_unchanged` test reads it with the current
//! library, so an archive a reader already has is proven to keep working.
//!
//! To regenerate it, take a program pinned to `rusqlite = "=0.40.2"` with the
//! `bundled-sqlcipher` feature, apply `PRAGMA key = "x'<64 hex chars>'"` with
//! the same 32 bytes, then write the same schema and pragmas.

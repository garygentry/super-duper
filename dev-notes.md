# Dev Notes

## Storage

The current engine uses embedded SQLite through `rusqlite`. The schema lives in
`crates/super-duper-core/src/storage/schema.sql` and is applied automatically when the engine opens
the database.

Useful places to start:

- `crates/super-duper-core/src/storage/sqlite.rs` - connection setup, pragmas, schema application
- `crates/super-duper-core/src/storage/queries.rs` - insert and query helpers
- `crates/super-duper-core/src/storage/models.rs` - plain Rust data models

## Local Runtime Files

The CLI writes runtime files in the current working directory by default:

- `super_duper.db`
- `content_hash_cache.db`
- `logs/sd.log`

These are local artifacts and should stay out of source control.

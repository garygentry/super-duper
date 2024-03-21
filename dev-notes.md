# Dev Notes

## Diesel Cheat Sheet
Setting up Diesel in clean environment

Create database and empty migrations directory
```bash
diesel setup
```
Create a new migration (with incremental DDL).  Only run this when modifying database schema
```bash
# Creates new up and down scripts in ./migrations/<migration_name>
diesel migration generate _<migration name>_
```

Apply the migrations to the database.  This will also update the schema.rs file.
```bash
diesel migration run
```

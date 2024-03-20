# super-duper

## Diesel commands

Create database and empty migrations 
```bash
diesel setup
```
Create a new migration (with incremental DDL)
```bash
# Creates new up and down scripts in ./migrations/<migration_name>
diesel migration generate _<migration name>_
```

Apply the migrations to the database
```bash
diesel migration run
```

# super-duper
Simple command line utility to find all duplicate files within a list of input directories and populate a db table with metadata for any duplicate files.  


## Installation 

### Database
This was built with a local Postres server but should be trivial to change it with configs

Set a `DATABASE_URL` environment variable with database connection string.  You can also
create a `.env` file in the root of the project with with a `DATABASE_URL` variable.

For example:
`DATABASE_URL=postgres://postgres:password@localhost/super_duper`

Install [diesel](https://diesel.rs/).  diesel is a ORM and query builder for Rust.  It is used to manage the database schema and migrations.

```bash
# Run setup to initialize database
diesel setup
# Apply the migration 
diesel migration run
```
## Configure Input Directories
Update `Config.toml` to include all of the directory paths to be scanned for duplicates.

For example:
`root_paths = ["C:/Temp", "../test-data/folder1", "../test-data/folder2"]`


## Run the program
```bash
cargo run
```

## Motivation
I have 20 years of files with countless machine builds where I archive off all of my important files and restart with just what I need and copied all of the stuff I don't need but can't bring my self to delete without going through it.  


## Approach
The basic idea is that the only reliable way to determine if two files are the same is to compare the file contents.  This is a slow process so the first step is to compare file sizes and only compare the contents of files that are the same size.  This is a simple way to reduce the number of files that need to be compared.  The next step is to compare the file contents.  This is done by hashing the file contents and comparing the hashes.  If the hashes are the same then the files are the same.  If the hashes are different then the files are different.  This is a reliable way to determine if two files are the same.  

However, with terabytes of files and many very large files such as videos or images, hashing every file is quite expensive.  So before hashing everything, we go through the file size structure and hash a small portion of the file to elliminate as many as we can before finally going through to hash the full contents of what's left.

Of course the hard part that an LLM can't help us with (right now anyway) is to go through and figure out which of the duplicated files you want to keep and how to organize it all. The thought was at some point to manipulate the data in a GUI, so the thought was to put all of the metadata into a database for now so that something else can operate on it later.  May end up moving all of this to a GUI app but building this gave me a chance to scale up the rust learning curve just a little bit and will be easier to port the logic over once it's working here (or build the GUI in rust too!).


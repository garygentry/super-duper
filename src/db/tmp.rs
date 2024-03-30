// use diesel::prelude::*;
// use diesel::result::Error;
// use std::env;

// diesel::table! {
//     dupe_file (id) {
//         id -> Int4,
//         canonical_name -> Text,
//     }
// }

// diesel::table! {
//     path_part (id) {
//         id -> Int4,
//     }
// }

// #[derive(Debug, Queryable, Selectable)]
// #[diesel(table_name = dupe_file)]
// pub struct DupeFileRead {
//     pub id: i32,
//     pub canonical_name: String,
// }

// #[derive(Debug, Queryable, Insertable)]
// #[diesel(table_name = path_part)]
// struct PathPart {
//     id: Option<i32>,
// }

// pub fn establish_connection() -> PgConnection {
//     let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
//     PgConnection::establish(&database_url)
//         .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
// }

// fn process_path_parts(connection: &mut PgConnection, canonical_name: &str) {
//     let mut path_parts = canonical_name.split('\\').collect::<Vec<&str>>();
//     let drive = path_parts.remove(0); // Remove the drive part
//     let drive_part = PathPart { id: None };
//     diesel::insert_into(path_part::table)
//         .values(&drive_part)
//         .execute(connection)
//         .expect("Error inserting drive part");
// }

// pub fn dupe_file_to_part_path() -> Result<(), Error> {
//     let mut connection = super::establish_connection();

//     let all_dupe_files = dupe_file::table
//         .load::<DupeFileRead>(&mut connection)
//         .expect("Error loading dupe_files");

//     for dupe_file_entry in all_dupe_files {
//         process_path_parts(&mut connection, &dupe_file_entry.canonical_name);
//     }

//     Ok(())
// }

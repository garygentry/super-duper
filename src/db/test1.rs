// use std::time::Instant;

// use super::schema::dupe_file::{self, canonical_name};
// use crate::model;
// use crossterm::style::Stylize;
// use diesel::prelude::*;
// use diesel::result::Error;

// pub fn test1() -> Result<(), Error> {
//     let connection = &mut super::establish_connection();

//     // Load all rows from the dupe_file table
//     let load_start_time = Instant::now();
//     let results = dupe_file::table
//         // .filter(canonical_name.similar_to(other))
//         // .limit(500)
//         .load::<model::DupeFileRead>(connection)
//         .expect("Error loading dupe_files");
//     let load_duration = load_start_time.elapsed();

//     let scan_start_time = Instant::now();
//     let mut count = 0;
//     for _file in results {
//         // println!("{}", file.canonical_name);
//         count += 1;
//     }
//     let scan_duration = scan_start_time.elapsed();
//     println!(
//         "Loaded {} records in {} ms, scanned in {} ms",
//         format_args!("{}", format!("{:.2}", count).green()),
//         format_args!("{}", format!("{:.2}", load_duration.as_millis()).green()),
//         format_args!("{}", format!("{:.2}", scan_duration.as_millis()).green()),
//     );

//     Ok(())
// }

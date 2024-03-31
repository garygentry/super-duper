use std::time::Instant;

use super::schema;
use crate::model;
use crossterm::style::Stylize;
use diesel::prelude::*;
use diesel::result::Error;

fn truncate_path_part(connection: &mut PgConnection) {
    diesel::sql_query("TRUNCATE TABLE path_part RESTART IDENTITY CASCADE")
        .execute(connection)
        .expect("Error truncating table");
}

fn get_canonical_name_parts(name: &str) -> Vec<&str> {
    // canonical name starts with \\?\.. which is surprisingly tricky to skip past
    // apparently because of they're escape vars and I'm too dumb to grok it
    // thus the hack here
    let parts: Vec<&str> = name.split('\\').collect();
    if parts.len() > 3 {
        let mut result = Vec::new();
        for &element in parts.iter().skip(3) {
            result.push(element); // Start extracting elements from the third position
        }
        result
    } else {
        panic!("Unexpected value for canonical_name: {}", name);
    }
}

fn process_path_parts(connection: &mut PgConnection, canonical_name: &str, file_size: i64) {
    let mut parent_id: Option<i32> = None;
    let parts = get_canonical_name_parts(canonical_name);
    for (i, part) in parts.iter().enumerate() {
        let part_type = if i == 0 {
            0
        } else if i == parts.len() - 1 {
            2
        } else {
            1
        };
        let canonical_name = parts[..=i].join("\\");
        let path_part = model::PathPart {
            id: None,
            canonical_name: canonical_name.clone(),
            file_size: if part_type == 2 { file_size } else { 0 },
            name: part.to_string(),
            parent_id,
            part_type,
        };
        let existing_part: Option<model::PathPartQuery> = schema::path_part::table
            .filter(schema::path_part::canonical_name.eq(&canonical_name))
            .first(connection)
            .optional()
            .expect("Error querying path_part");
        match existing_part {
            Some(mut existing_part) => {
                existing_part.file_size += file_size;
                diesel::update(schema::path_part::table.find(existing_part.id))
                    .set(schema::path_part::file_size.eq(existing_part.file_size))
                    .execute(connection)
                    .expect("Error updating path_part");
                parent_id = Some(existing_part.id);
            }
            None => {
                let inserted_id: i32 = diesel::insert_into(schema::path_part::table)
                    .values(&path_part)
                    .returning(schema::path_part::id)
                    .get_result(connection)
                    .expect("Error inserting into path_part");
                parent_id = Some(inserted_id);
                if part_type != 2 {
                    diesel::update(schema::path_part::table.find(inserted_id))
                        .set(schema::path_part::file_size.eq(file_size))
                        .execute(connection)
                        .expect("Error updating path_part");
                }
            }
        }
    }
}

pub fn dupe_file_to_part_path() -> Result<(), Error> {
    let mut connection = super::establish_connection();

    // Load all rows from the dupe_file table
    let load_start_time = Instant::now();
    let all_dupe_files = schema::dupe_file::table
        // .filter(
        //     schema::dupe_file::id
        //         .eq(130804)
        //         .or(schema::dupe_file::id.eq(74182))
        //         .or(schema::dupe_file::id.eq(270256))
        //         .or(schema::dupe_file::id.eq(74182)),
        // )
        // .limit(5000)
        .load::<model::DupeFileRead>(&mut connection)
        .expect("Error loading dupe_files");
    let load_duration = load_start_time.elapsed();

    let scan_start_time = Instant::now();
    let mut count = 0;

    truncate_path_part(&mut connection);

    for dupe_file_entry in all_dupe_files {
        process_path_parts(
            &mut connection,
            &dupe_file_entry.canonical_name,
            dupe_file_entry.file_size,
        );
        count += 1;
    }

    let scan_duration = scan_start_time.elapsed();
    println!(
        "Loaded {} records in {} ms, scanned in {} ms",
        format_args!("{}", format!("{:.2}", count).green()),
        format_args!("{}", format!("{:.2}", load_duration.as_millis()).green()),
        format_args!("{}", format!("{:.2}", scan_duration.as_millis()).green()),
    );

    Ok(())
}

#![allow(dead_code)]

use std::time::Instant;

use super::schema;
use crate::{
    db::POSTGRES_MAX_PARAMETERS,
    model::{self, PathPart},
};
use ahash::AHashMap;
use anyhow::Result;
use crossterm::style::Stylize;
use csv::Writer;
use diesel::prelude::*;
use std::error::Error;
use tracing::error;

pub fn write_path_parts_to_csv(
    path_parts: &[PathPart],
    file_path: &str,
) -> Result<(), Box<dyn Error>> {
    let mut writer = Writer::from_path(file_path)?;

    // Write headers
    writer.write_record([
        "id",
        "canonical_name",
        "name",
        "file_size",
        "parent_id",
        "part_type",
    ])?;

    // Write data
    for part in path_parts {
        writer.write_record(&[
            part.id.to_string(),
            part.canonical_name.clone(),
            part.name.clone(),
            part.file_size.to_string(),
            part.parent_id.map_or("".to_string(), |id| id.to_string()),
            part.part_type.to_string(),
        ])?;
    }

    writer.flush()?;

    Ok(())
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

fn process_path_parts(
    path_parts: &mut AHashMap<String, model::PathPart>,
    canonical_name: &str,
    file_size: i64,
    id_counter: &mut i32,
) -> Vec<model::PathPart> {
    let mut parent_id: Option<i32> = None;
    let parts = get_canonical_name_parts(canonical_name);
    let mut path_part_vec = Vec::new();

    for (i, part) in parts.iter().enumerate() {
        let part_type = if i == 0 {
            0
        } else if i == parts.len() - 1 {
            2
        } else {
            1
        };
        let canonical_name = parts[..=i].join("\\");

        match path_parts.get_mut(&canonical_name) {
            Some(existing_part) => {
                existing_part.file_size += file_size;
                parent_id = Some(existing_part.id);
            }
            None => {
                let path_part = model::PathPart {
                    id: *id_counter,
                    canonical_name: canonical_name.clone(),
                    file_size,
                    name: part.to_string(),
                    parent_id,
                    part_type,
                };
                parent_id = Some(*id_counter);
                *id_counter += 1;
                path_parts.insert(canonical_name.clone(), path_part.clone());
                path_part_vec.push(path_part);
            }
        }
    }

    path_part_vec
}

pub fn dupe_file_to_part_path() -> Result<(), anyhow::Error> {
    let mut connection = super::establish_connection();

    // Load all rows from the dupe_file table
    let all_dupe_files = schema::dupe_file::table
        // .filter(
        //     schema::dupe_file::id
        //         .eq(130804)
        //         .or(schema::dupe_file::id.eq(74182))
        //         .or(schema::dupe_file::id.eq(270256))
        //         .or(schema::dupe_file::id.eq(74182)),
        // )
        // .limit(500000)
        .load::<model::DupeFileRead>(&mut connection)
        .expect("Error loading dupe_files");

    let mut count = 0;

    let file_to_part_factor = 1.25;
    let map_capacity: usize = (all_dupe_files.len() as f64 * file_to_part_factor).floor() as usize;

    let mut path_parts: AHashMap<String, model::PathPart> = AHashMap::with_capacity(map_capacity);
    let mut id_counter = 1;

    let proc_start_time = Instant::now();
    for dupe_file_entry in all_dupe_files {
        process_path_parts(
            &mut path_parts,
            &dupe_file_entry.canonical_name,
            dupe_file_entry.file_size,
            &mut id_counter,
        );
        count += 1;
    }
    let proc_duration = proc_start_time.elapsed();

    // let path_part_vec: Vec<model::PathPartQuery> = path_parts.values().cloned().collect();
    // let _ = write_path_parts_to_csv(&path_part_vec, "./path_parts.csv");
    // Postgres max parameters = 65535, max chunk size = 65535 / [col count].  6 cols = 10922 max
    // let insert_chunk_size: usize = 10000;

    // for now, build path_part from scratch
    // super::truncate_path_part(&mut connection);
    super::truncate_path_part();

    let insert_chunk_size: usize = POSTGRES_MAX_PARAMETERS / model::PATH_PART_FIELD_COUNT;
    let insert_start_time = Instant::now();
    let rows_inserted = match insert_path_parts(&mut connection, &path_parts, &insert_chunk_size) {
        Ok(rows_inserted) => rows_inserted,
        Err(err) => {
            error!("insert_path_parts error {}", err);

            let err = anyhow::Error::new(err);
            let err = err.context("Error inserting path parts");
            return Err(err);
        }
    };
    let insert_duration = insert_start_time.elapsed();
    let proc_rows_per_sec = rows_inserted as f64 / proc_duration.as_secs() as f64;

    println!(
        "Processed {} records in {} ms ({} rows per second)\nInserted {} rows to database in {} ms (insert chunk size={})",
        format_args!("{}", format!("{:.2}", count).green()),
        format_args!("{}", format!("{:.2}", proc_duration.as_millis()).green()),
        format_args!("{}", format!("{:.2}", proc_rows_per_sec).green()),
        format_args!("{}", format!("{:.2}", rows_inserted).green()),
        format_args!("{}", format!("{:.2}", insert_duration.as_millis()).green()),
        format_args!("{}", format!("{:.2}", insert_chunk_size).green()),
    );

    Ok(())
}

pub fn insert_path_parts(
    connection: &mut PgConnection,
    path_parts: &AHashMap<String, PathPart>,
    chunk_size: &usize,
) -> Result<usize, diesel::result::Error> {
    let values: Vec<_> = path_parts.values().collect();

    // // let chunk_size = 1000;
    let mut rows_added = 0;

    for chunk in values.chunks(*chunk_size) {
        let chunk: Vec<_> = chunk.to_vec();
        let rows = diesel::insert_into(schema::path_part::table)
            .values(chunk)
            .execute(connection)?;
        rows_added += rows;
    }

    Ok(rows_added)
}

// pub fn insert_path_parts(
//     connection: &mut PgConnection,
//     path_parts: &HashMap<String, PathPartQuery>,
// ) -> Result<usize, diesel::result::Error> {
//     let values: Vec<_> = path_parts.values().collect();

//     let rows = diesel::insert_into(schema::path_part::table)
//         .values(values)
//         .execute(connection)?;

//     Ok(rows)
// }

// fn truncate_path_part(connection: &mut PgConnection) {
//     diesel::sql_query("TRUNCATE TABLE path_part RESTART IDENTITY CASCADE")
//         .execute(connection)
//         .expect("Error truncating table");
// }

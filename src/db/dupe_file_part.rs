use std::collections::HashMap;

use super::sd_pg::*;
use super::schema;
use diesel::prelude::*;
use super::dupe_file::DupeFile;

#[derive(Debug, Queryable, Insertable, Selectable, Clone)]
#[diesel(table_name = schema::dupe_file_part)]
pub struct DupeFilePart {
    pub id: i32,
    pub canonical_path: String,
    pub name: String,
    pub file_size: i64,
    pub parent_id: Option<i32>,
    pub part_type: i32,
    pub session_id: i32,
}

pub const PATH_PART_FIELD_COUNT: usize = 7;

const FILE_TO_PART_FACTOR: f64 = 1.25;

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
    path_parts: &mut HashMap<String, DupeFilePart>,
    canonical_path: &str,
    file_size: i64,
    id_counter: &mut i32,
    session_id: &i32
) -> Vec<DupeFilePart> {
    let mut parent_id: Option<i32> = None;
    let parts = get_canonical_name_parts(canonical_path);
    let mut path_part_vec = Vec::new();

    for (i, part) in parts.iter().enumerate() {
        let part_type = if i == 0 { 0 } else if i == parts.len() - 1 { 2 } else { 1 };
        let canonical_name = parts[..=i].join("\\");

        match path_parts.get_mut(&canonical_name) {
            Some(existing_part) => {
                existing_part.file_size += file_size;
                parent_id = Some(existing_part.id);
            }
            None => {
                let path_part = DupeFilePart {
                    id: *id_counter,
                    canonical_path: canonical_name.clone(),
                    file_size,
                    name: part.to_string(),
                    parent_id,
                    part_type,
                    session_id: *session_id,
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

impl DupeFilePart {
    pub fn parts_from_dupe_files(dupe_files: &[DupeFile], session_id: &i32) -> Vec<DupeFilePart> {
        let map_capacity: usize = (
            (dupe_files.len() as f64) * FILE_TO_PART_FACTOR
        ).floor() as usize;

        let mut path_parts: HashMap<String, DupeFilePart> = HashMap::with_capacity(map_capacity);
        let mut id_counter = 1;

        for dupe_file in dupe_files {
            process_path_parts(
                &mut path_parts,
                &dupe_file.canonical_path,
                dupe_file.file_size,
                &mut id_counter,
                session_id
            );
        }

        let values: Vec<DupeFilePart> = path_parts.values().cloned().collect();
        values

        // let _empty_vec: Vec<DupeFilePart> = vec![];
        // _empty_vec
        // let values: Vec<_> = path_parts.values().collect();
        // values
    }

    pub fn insert_dupe_file_parts(
        connection: &mut PgConnection,
        dupe_file_parts: &[DupeFilePart]
    ) -> Result<usize, diesel::result::Error> {
        let chunk_size: usize = POSTGRES_MAX_PARAMETERS / PATH_PART_FIELD_COUNT;

        let mut rows_added = 0;

        for chunk in dupe_file_parts.chunks(chunk_size) {
            let rows = diesel
                ::insert_into(schema::dupe_file_part::table)
                .values(chunk)
                .execute(connection)?;

            rows_added += rows;
        }

        Ok(rows_added)
    }
}

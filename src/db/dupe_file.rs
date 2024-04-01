use super::schema::dupe_file;
use crate::model;
use diesel::prelude::*;
use diesel::result::Error;

pub fn insert_file_info_vec(file_info_vector: Vec<model::FileInfo>) -> Result<usize, Error> {
    let mut connection = super::establish_connection();

    let chunk_size: usize = super::POSTGRES_MAX_PARAMETERS / model::DUPE_FILE_FIELD_COUNT;

    let mut rows_added = 0;

    let values = file_info_vector;

    for chunk in values.chunks(chunk_size) {
        let rows = diesel::insert_into(dupe_file::table)
            .values(chunk)
            .execute(&mut connection)?;
        rows_added += rows;
    }

    // let _ = rows_added;
    // for file_info in file_info_vector {
    //     diesel::insert_into(dupe_file::table)
    //         .values(&file_info)
    //         .execute(&mut connection)
    //         .map_err(|err| {
    //             eprintln!("Error saving file info: {}", err);
    //             err
    //         })?;
    // }

    Ok(rows_added)
}

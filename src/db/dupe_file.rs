use super::schema::dupe_file;
use crate::model;
use diesel::prelude::*;
use diesel::result::Error;

pub fn insert_file_info_vec(file_info_vector: Vec<model::FileInfo>) -> Result<(), Error> {
    let mut connection = super::establish_connection();

    for file_info in file_info_vector {
        diesel::insert_into(dupe_file::table)
            .values(&file_info)
            .execute(&mut connection)
            .map_err(|err| {
                eprintln!("Error saving file info: {}", err);
                err
            })?;
    }

    Ok(())
}

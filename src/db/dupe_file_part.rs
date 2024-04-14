use super::schema;
use diesel::prelude::*;

#[derive(Debug, Queryable, Insertable, Selectable, Clone)]
#[diesel(table_name = schema::dupe_file_part)]
pub struct PathPart {
    pub id: i32,
    pub canonical_path: String,
    pub name: String,
    pub file_size: i64,
    pub parent_id: Option<i32>,
    pub part_type: i32,
}

pub const _PATH_PART_FIELD_COUNT: usize = 6;

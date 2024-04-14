#![allow(dead_code)]
use diesel::pg::PgConnection;
use diesel::prelude::*;
use std::env;
use tracing::debug;

pub const POSTGRES_MAX_PARAMETERS: usize = 65535;

pub fn establish_connection() -> PgConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn truncate_tables() {
    truncate_dupe_file();
    truncate_path_part();
}

pub fn truncate_dupe_file() {
    truncate_table("dupe_file");
}

pub fn truncate_path_part() {
    truncate_table("path_part");
}

fn truncate_table(table_name: &str) {
    let mut connection = establish_connection();

    debug!("Truncating table: {}", table_name);
    diesel::sql_query(format!(
        "TRUNCATE TABLE {} RESTART IDENTITY CASCADE",
        table_name
    ))
    .execute(&mut connection)
    .expect("Error truncating table");
}

// pub fn truncate_path_part(&mut connection: PgConnection) {
//     let connection = connection.get_or_insert_with(establish_connection);

//     diesel::sql_query("TRUNCATE TABLE path_part RESTART IDENTITY CASCADE")
//         .execute(connection)
//         .expect("Error truncating table");
// }

// pub fn truncate_table(connection: Option<&mut PgConnection>, table_name: &str) {
//     let binding = establish_connection();
//     let connection = match connection {
//         Some(connection) => connection,
//         None => &mut binding,
//     };
//     let sql = format!("TRUNCATE TABLE {} RESTART IDENTITY CASCADE", &table_name);

//     println!("SQL: {}", sql);

//     diesel::sql_query(sql)
//         .execute(connection)
//         .expect("Error truncating table");
// }

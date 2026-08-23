use rusqlite::types::ValueRef;
use serde_json::{json, Map, Value};
use std::{env, fs, path::Path};
use super_duper_core::storage::Database;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 {
        return Err(
            "usage: windows_ambiguous_start_evidence <database> <operation-id> <output>".into(),
        );
    }
    let database_path = &arguments[0];
    let operation_id = arguments[1].parse::<i64>()?;
    let output_path = &arguments[2];
    let db = Database::open(database_path)?;
    let schema_version: i64 = db
        .connection()
        .query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let review = db.get_recovery_review(operation_id)?;
    let document = json!({
        "schemaVersion": 1,
        "databaseSchemaVersion": schema_version,
        "recycleOperationId": operation_id,
        "sourceEvidence": {
            "operation": query_rows(db.connection(), "SELECT * FROM recycle_operation WHERE id = ?1 ORDER BY id", operation_id)?,
            "batches": query_rows(db.connection(), "SELECT * FROM recycle_operation_batch WHERE recycle_operation_id = ?1 ORDER BY id", operation_id)?,
            "items": query_rows(db.connection(), "SELECT * FROM recycle_operation_item WHERE recycle_operation_id = ?1 ORDER BY id", operation_id)?,
            "recovery": query_rows(db.connection(), "SELECT * FROM recycle_operation_recovery WHERE recycle_operation_id = ?1 ORDER BY id", operation_id)?,
            "reports": query_rows(db.connection(), "SELECT * FROM recycle_operation_report WHERE recycle_operation_id = ?1 ORDER BY id", operation_id)?,
        },
        "observations": query_rows(db.connection(), "SELECT * FROM recovery_review_observation WHERE recycle_operation_id = ?1 ORDER BY id", operation_id)?,
        "derivedReview": {
            "state": review.state.as_str(),
            "unknownItemCount": review.unknown_item_count,
            "observedItemCount": review.observed_item_count,
        },
        "globalCounts": {
            "operations": scalar(db.connection(), "SELECT COUNT(*) FROM recycle_operation")?,
            "batches": scalar(db.connection(), "SELECT COUNT(*) FROM recycle_operation_batch")?,
            "items": scalar(db.connection(), "SELECT COUNT(*) FROM recycle_operation_item")?,
            "recoveryRows": scalar(db.connection(), "SELECT COUNT(*) FROM recycle_operation_recovery")?,
            "reports": scalar(db.connection(), "SELECT COUNT(*) FROM recycle_operation_report")?,
            "observations": scalar(db.connection(), "SELECT COUNT(*) FROM recovery_review_observation")?,
        },
    });
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, serde_json::to_vec_pretty(&document)?)?;
    Ok(())
}

fn scalar(connection: &rusqlite::Connection, sql: &str) -> rusqlite::Result<i64> {
    connection.query_row(sql, [], |row| row.get(0))
}

fn query_rows(
    connection: &rusqlite::Connection,
    sql: &str,
    operation_id: i64,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(sql)?;
    let names = statement
        .column_names()
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let rows = statement
        .query_map([operation_id], |row| {
            let mut value = Map::new();
            for (index, name) in names.iter().enumerate() {
                value.insert(name.clone(), sqlite_value(row.get_ref(index)?));
            }
            Ok(Value::Object(value))
        })?
        .collect();
    rows
}

fn sqlite_value(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::from(value),
        ValueRef::Real(value) => Value::from(value),
        ValueRef::Text(value) => Value::from(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => Value::from(hex(value)),
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

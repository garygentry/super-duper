use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Distribution {
    available: usize,
    unavailable: usize,
    minimum: Option<u64>,
    p50: Option<u64>,
    p95: Option<u64>,
    p99: Option<u64>,
    maximum: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductEvidence {
    run_id: i64,
    status: String,
    repeat_cache_policy: String,
    parameter_signature_sha256: String,
    root_identity_sha256: Vec<String>,
    files_discovered: u64,
    bytes_discovered: u64,
    files_hashed: u64,
    duplicate_file_groups: u64,
    duplicate_folder_groups: u64,
    wasted_bytes: u64,
    warning_count: u64,
    warning_occurrence_count: u64,
    warning_accounting_complete: bool,
    file_result_sha256: String,
    folder_result_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceEvidence {
    device_key_sha256: String,
    sample_count: usize,
    unavailable_counter_total: u64,
    read_bytes_per_second: Distribution,
    read_iops_millis: Distribution,
    average_read_latency_micros: Distribution,
    active_millis_per_second: Distribution,
    queue_depth_millis: Distribution,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusEvidence {
    status_run_id: i64,
    state: String,
    metrics_contract_version: u64,
    last_monotonic_nanos: u64,
    last_sequence: u64,
    counters: BTreeMap<String, u64>,
    flush_count: u64,
    flush_payload_bytes: u64,
    host_sample_count: usize,
    host_unavailable_counter_total: u64,
    process_cpu_nanos: Distribution,
    process_private_bytes: Distribution,
    process_working_set_bytes: Distribution,
    process_peak_working_set_bytes: Distribution,
    process_read_operations: Distribution,
    process_read_bytes: Distribution,
    process_write_operations: Distribution,
    process_write_bytes: Distribution,
    devices: Vec<DeviceEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotEvidence {
    schema_version: u32,
    product: ProductEvidence,
    status: StatusEvidence,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let product_path = args.next().ok_or("missing product database path")?;
    let status_path = args.next().ok_or("missing status database path")?;
    let run_id: i64 = args.next().ok_or("missing run id")?.parse()?;
    if args.next().is_some() || run_id <= 0 {
        return Err(
            "usage: sop9_evidence_snapshot <product-db> <status-db> <positive-run-id>".into(),
        );
    }

    let product = open_read_only(product_path)?;
    let status = open_read_only(status_path)?;
    let evidence = SnapshotEvidence {
        schema_version: 1,
        product: product_evidence(&product, run_id)?,
        status: status_evidence(&status, run_id)?,
    };
    serde_json::to_writer(std::io::stdout().lock(), &evidence)?;
    println!();
    Ok(())
}

fn open_read_only(path: impl AsRef<Path>) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

fn product_evidence(
    connection: &Connection,
    run_id: i64,
) -> Result<ProductEvidence, Box<dyn Error>> {
    let row = connection.query_row(
        "SELECT status, parameters_json, files_discovered, bytes_discovered, files_hashed,
                duplicate_file_groups, duplicate_folder_groups, wasted_bytes, warning_count
           FROM scan_run WHERE id = ?1",
        [run_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, u64>(4)?,
                row.get::<_, u64>(5)?,
                row.get::<_, u64>(6)?,
                row.get::<_, u64>(7)?,
                row.get::<_, u64>(8)?,
            ))
        },
    )?;
    let warning_occurrence_count = connection.query_row(
        "SELECT COALESCE(SUM(occurrence_count), 0) FROM run_warning_aggregate WHERE run_id = ?1",
        [run_id],
        |row| row.get::<_, u64>(0),
    )?;
    let parameters: serde_json::Value = serde_json::from_str(&row.1)?;
    let repeat_cache_policy = parameters
        .get("repeat_cache_policy")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("revalidate_content")
        .to_owned();
    let root_identity_sha256 = parameters
        .get("roots")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(|root| sha256_text(&root.to_uppercase()))
        .collect();
    Ok(ProductEvidence {
        run_id,
        status: row.0,
        repeat_cache_policy,
        parameter_signature_sha256: sha256_text(&row.1),
        root_identity_sha256,
        files_discovered: row.2,
        bytes_discovered: row.3,
        files_hashed: row.4,
        duplicate_file_groups: row.5,
        duplicate_folder_groups: row.6,
        wasted_bytes: row.7,
        warning_count: row.8,
        warning_occurrence_count,
        warning_accounting_complete: row.8 == warning_occurrence_count,
        file_result_sha256: digest_query(
            connection,
            "SELECT group_row.content_hash, group_row.file_size, group_row.file_count,
                    group_row.wasted_bytes, file_row.canonical_path, file_row.file_size,
                    COALESCE(file_row.file_identity, '')
               FROM duplicate_group group_row
               JOIN duplicate_group_member member ON member.group_id = group_row.id
               JOIN scanned_file file_row ON file_row.id = member.file_id
              WHERE group_row.run_id = ?1
              ORDER BY group_row.content_hash, group_row.file_size,
                       file_row.canonical_path COLLATE BINARY",
            run_id,
            7,
        )?,
        folder_result_sha256: digest_query(
            connection,
            "SELECT group_row.structural_fingerprint, group_row.verified_fingerprint,
                    group_row.total_size, group_row.file_count, group_row.folder_count,
                    group_row.is_suppressed, directory_row.path
               FROM duplicate_folder_group group_row
               JOIN duplicate_folder_group_member member ON member.group_id = group_row.id
               JOIN directory_node directory_row ON directory_row.id = member.directory_id
              WHERE group_row.run_id = ?1
              ORDER BY group_row.verified_fingerprint, group_row.structural_fingerprint,
                       group_row.is_suppressed, directory_row.path COLLATE BINARY",
            run_id,
            7,
        )?,
    })
}

fn digest_query(
    connection: &Connection,
    sql: &str,
    run_id: i64,
    column_count: usize,
) -> rusqlite::Result<String> {
    let mut hasher = Sha256::new();
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query([run_id])?;
    while let Some(row) = rows.next()? {
        for index in 0..column_count {
            let value = row.get_ref(index)?;
            let bytes: Vec<u8> = match value {
                rusqlite::types::ValueRef::Null => b"N".to_vec(),
                rusqlite::types::ValueRef::Integer(value) => format!("I{value}").into_bytes(),
                rusqlite::types::ValueRef::Real(value) => format!("R{value:.17}").into_bytes(),
                rusqlite::types::ValueRef::Text(value) => {
                    let mut normalized = b"T".to_vec();
                    normalized.extend(value.iter().map(u8::to_ascii_uppercase));
                    normalized
                }
                rusqlite::types::ValueRef::Blob(value) => {
                    let mut normalized = b"B".to_vec();
                    normalized.extend_from_slice(value);
                    normalized
                }
            };
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn status_evidence(
    connection: &Connection,
    product_run_id: i64,
) -> Result<StatusEvidence, Box<dyn Error>> {
    let header = connection.query_row(
        "SELECT id, state, metrics_contract_version, last_monotonic_nanos, last_sequence
           FROM status_run WHERE product_run_id = ?1 ORDER BY id DESC LIMIT 1",
        [product_run_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, u64>(4)?,
            ))
        },
    )?;
    let status_run_id = header.0;
    let mut counters = BTreeMap::new();
    let mut statement = connection.prepare(
        "SELECT metric, value FROM status_counter WHERE run_id = ?1 AND phase = 'overall' ORDER BY metric",
    )?;
    for row in statement.query_map([status_run_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
    })? {
        let (name, value) = row?;
        counters.insert(name, value);
    }
    let (flush_count, flush_payload_bytes) = connection.query_row(
        "SELECT COUNT(*), COALESCE(SUM(length(payload_json)), 0) FROM status_flush WHERE run_id = ?1",
        [status_run_id],
        |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?)),
    )?;
    let host_rows = collect_host_rows(connection, status_run_id)?;
    let host_unavailable_counter_total = host_rows.iter().map(|row| row[8].unwrap_or(0)).sum();
    let mut device_keys = Vec::new();
    let mut statement = connection.prepare(
        "SELECT DISTINCT device_key FROM status_device_sample WHERE run_id = ?1 ORDER BY device_key",
    )?;
    for row in statement.query_map([status_run_id], |row| row.get::<_, String>(0))? {
        device_keys.push(row?);
    }
    let mut devices = Vec::with_capacity(device_keys.len());
    for device_key in device_keys {
        let rows = collect_device_rows(connection, status_run_id, &device_key)?;
        devices.push(DeviceEvidence {
            device_key_sha256: sha256_text(&device_key),
            sample_count: rows.len(),
            unavailable_counter_total: rows.iter().map(|row| row[5].unwrap_or(0)).sum(),
            read_bytes_per_second: distribution(rows.iter().map(|row| row[0]).collect()),
            read_iops_millis: distribution(rows.iter().map(|row| row[1]).collect()),
            average_read_latency_micros: distribution(rows.iter().map(|row| row[2]).collect()),
            active_millis_per_second: distribution(rows.iter().map(|row| row[3]).collect()),
            queue_depth_millis: distribution(rows.iter().map(|row| row[4]).collect()),
        });
    }
    Ok(StatusEvidence {
        status_run_id,
        state: header.1,
        metrics_contract_version: header.2,
        last_monotonic_nanos: header.3,
        last_sequence: header.4,
        counters,
        flush_count,
        flush_payload_bytes,
        host_sample_count: host_rows.len(),
        host_unavailable_counter_total,
        process_cpu_nanos: distribution(host_rows.iter().map(|row| row[0]).collect()),
        process_private_bytes: distribution(host_rows.iter().map(|row| row[1]).collect()),
        process_working_set_bytes: distribution(host_rows.iter().map(|row| row[2]).collect()),
        process_peak_working_set_bytes: distribution(host_rows.iter().map(|row| row[3]).collect()),
        process_read_operations: distribution(host_rows.iter().map(|row| row[4]).collect()),
        process_read_bytes: distribution(host_rows.iter().map(|row| row[5]).collect()),
        process_write_operations: distribution(host_rows.iter().map(|row| row[6]).collect()),
        process_write_bytes: distribution(host_rows.iter().map(|row| row[7]).collect()),
        devices,
    })
}

type HostRow = [Option<u64>; 9];
type DeviceRow = [Option<u64>; 6];

fn collect_host_rows(connection: &Connection, run_id: i64) -> rusqlite::Result<Vec<HostRow>> {
    let mut statement = connection.prepare(
        "SELECT process_cpu_nanos, process_private_bytes, process_working_set_bytes,
                process_peak_working_set_bytes, process_read_operations, process_read_bytes,
                process_write_operations, process_write_bytes, unavailable_counter_count
           FROM status_host_sample WHERE run_id = ?1 ORDER BY sequence LIMIT 100000",
    )?;
    let rows = statement
        .query_map([run_id], |row| {
            Ok([
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ])
        })?
        .collect();
    rows
}

fn collect_device_rows(
    connection: &Connection,
    run_id: i64,
    device_key: &str,
) -> rusqlite::Result<Vec<DeviceRow>> {
    let mut statement = connection.prepare(
        "SELECT read_bytes_per_second, read_iops_millis, average_read_latency_micros,
                active_millis_per_second, queue_depth_millis, unavailable_counter_count
           FROM status_device_sample
          WHERE run_id = ?1 AND device_key = ?2 ORDER BY sequence LIMIT 100000",
    )?;
    let rows = statement
        .query_map(rusqlite::params![run_id, device_key], |row| {
            Ok([
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ])
        })?
        .collect();
    rows
}

fn distribution(values: Vec<Option<u64>>) -> Distribution {
    let unavailable = values.iter().filter(|value| value.is_none()).count();
    let mut available: Vec<u64> = values.into_iter().flatten().collect();
    available.sort_unstable();
    Distribution {
        available: available.len(),
        unavailable,
        minimum: available.first().copied(),
        p50: percentile(&available, 50),
        p95: percentile(&available, 95),
        p99: percentile(&available, 99),
        maximum: available.last().copied(),
    }
}

fn percentile(values: &[u64], percentile: usize) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let rank = (values.len() * percentile).div_ceil(100).saturating_sub(1);
    values.get(rank).copied()
}

fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

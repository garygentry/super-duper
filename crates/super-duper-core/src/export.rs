//! Versioned export documents for duplicate file groups and session data
//! (`docs/export-format-v1.md`). The CLI's `export` subcommand is the only current producer; a
//! future Windows app export (issue #30) must reuse these field names and rules, or the format
//! version bumps.

use crate::Error;
use crate::path_spelling::to_plain_spelling;
use crate::storage::Database;
use serde::Serialize;

/// The export document format. Additive fields are allowed within a version without bumping it.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupsExport {
    pub format_version: u32,
    pub generated_at: String,
    pub run_id: i64,
    pub groups: Vec<DuplicateGroupExportEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupExportEntry {
    pub id: i64,
    /// Lowercase 16-digit hex XxHash64 of the group's content, the grouping key.
    pub content_hash: String,
    pub file_size: i64,
    pub file_count: i64,
    pub wasted_bytes: i64,
    pub members: Vec<DuplicateGroupMemberExport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupMemberExport {
    /// Plain spelling (`docs/export-format-v1.md`); not an identity value.
    pub path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub root_path: String,
    pub relative_path: String,
    pub drive_letter: String,
    pub file_size: i64,
    /// Unix nanoseconds, as a string: it exceeds the safe integer range of a JSON number.
    pub last_modified_unix_nanos: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsExport {
    pub format_version: u32,
    pub generated_at: String,
    pub sessions: Vec<SessionExportEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExportEntry {
    pub id: i64,
    pub name: String,
    pub roots: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub cloud_policy: String,
    pub created_at: String,
    pub updated_at: String,
    pub runs: Vec<SessionRunExport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRunExport {
    pub id: i64,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub files_discovered: i64,
    pub bytes_discovered: i64,
    pub files_hashed: i64,
    pub duplicate_file_groups: i64,
    pub duplicate_folder_groups: i64,
    pub wasted_bytes: i64,
    pub warning_count: i64,
    pub excluded_subtree_count: i64,
    pub engine_version: String,
}

/// Exports every duplicate file group and its members for `run_id`, or the latest completed run
/// when `None`.
pub fn export_duplicate_groups(
    db: &Database,
    run_id: Option<i64>,
    format: ExportFormat,
) -> Result<String, Error> {
    let run_id = match run_id {
        Some(id) => {
            db.get_scan_run(id).map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("run {id} not found")),
                other => Error::Database(other),
            })?;
            id
        }
        None => db
            .get_latest_completed_run_id()?
            .ok_or_else(|| Error::Other("no completed run is available for export".to_owned()))?,
    };
    let groups = db
        .get_duplicate_groups(run_id, 0, i64::MAX)?
        .into_iter()
        .map(|group| {
            let members = db
                .get_files_in_group(group.id)?
                .into_iter()
                .map(|file| DuplicateGroupMemberExport {
                    path: to_plain_spelling(&file.canonical_path),
                    file_name: file.file_name,
                    parent_dir: to_plain_spelling(&file.parent_dir),
                    root_path: file.root_path,
                    relative_path: file.relative_path,
                    drive_letter: file.drive_letter,
                    file_size: file.file_size,
                    last_modified_unix_nanos: file.last_modified.to_string(),
                })
                .collect();
            Ok(DuplicateGroupExportEntry {
                id: group.id,
                content_hash: format!("{:016x}", group.content_hash as u64),
                file_size: group.file_size,
                file_count: group.file_count,
                wasted_bytes: group.wasted_bytes,
                members,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let document = DuplicateGroupsExport {
        format_version: EXPORT_FORMAT_VERSION,
        generated_at: chrono::Utc::now().to_rfc3339(),
        run_id,
        groups,
    };
    render(&document, format, duplicate_groups_csv)
}

/// Exports session definitions and their run history: every session, or just `session_id` when
/// given.
pub fn export_sessions(
    db: &Database,
    session_id: Option<i64>,
    format: ExportFormat,
) -> Result<String, Error> {
    let sessions = match session_id {
        Some(id) => vec![db.get_session(id).map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("session {id} not found")),
            other => Error::Database(other),
        })?],
        None => db.list_sessions(0, i64::MAX)?.0,
    };
    let sessions = sessions
        .into_iter()
        .map(|session| {
            let roots: Vec<String> = serde_json::from_str(&session.roots_json)
                .map_err(|error| Error::Other(format!("invalid stored roots_json: {error}")))?;
            let ignore_patterns: Vec<String> = serde_json::from_str(&session.ignore_patterns_json)
                .map_err(|error| {
                    Error::Other(format!("invalid stored ignore_patterns_json: {error}"))
                })?;
            let runs = db
                .list_session_runs(session.id, 0, i64::MAX)?
                .0
                .into_iter()
                .map(|run| SessionRunExport {
                    id: run.id,
                    status: run.status,
                    created_at: run.created_at,
                    started_at: run.started_at,
                    completed_at: run.completed_at,
                    files_discovered: run.files_discovered,
                    bytes_discovered: run.bytes_discovered,
                    files_hashed: run.files_hashed,
                    duplicate_file_groups: run.duplicate_file_groups,
                    duplicate_folder_groups: run.duplicate_folder_groups,
                    wasted_bytes: run.wasted_bytes,
                    warning_count: run.warning_count,
                    excluded_subtree_count: run.excluded_subtree_count,
                    engine_version: run.engine_version,
                })
                .collect();
            Ok(SessionExportEntry {
                id: session.id,
                name: session.name,
                roots,
                ignore_patterns,
                cloud_policy: session.cloud_policy,
                created_at: session.created_at,
                updated_at: session.updated_at,
                runs,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let document = SessionsExport {
        format_version: EXPORT_FORMAT_VERSION,
        generated_at: chrono::Utc::now().to_rfc3339(),
        sessions,
    };
    render(&document, format, sessions_csv)
}

fn render<T: Serialize>(
    document: &T,
    format: ExportFormat,
    to_csv: impl FnOnce(&T) -> String,
) -> Result<String, Error> {
    match format {
        ExportFormat::Json => serde_json::to_string_pretty(document)
            .map_err(|error| Error::Other(format!("failed to serialize export: {error}"))),
        ExportFormat::Csv => Ok(to_csv(document)),
    }
}

const DUPLICATE_GROUP_CSV_HEADER: &[&str] = &[
    "groupId",
    "contentHash",
    "groupFileSize",
    "groupFileCount",
    "wastedBytes",
    "path",
    "fileName",
    "parentDir",
    "rootPath",
    "relativePath",
    "driveLetter",
    "fileSize",
    "lastModifiedUnixNanos",
];

fn duplicate_groups_csv(document: &DuplicateGroupsExport) -> String {
    let mut csv = csv_row(DUPLICATE_GROUP_CSV_HEADER);
    for group in &document.groups {
        for member in &group.members {
            csv.push_str(&csv_row(&[
                group.id.to_string(),
                group.content_hash.clone(),
                group.file_size.to_string(),
                group.file_count.to_string(),
                group.wasted_bytes.to_string(),
                member.path.clone(),
                member.file_name.clone(),
                member.parent_dir.clone(),
                member.root_path.clone(),
                member.relative_path.clone(),
                member.drive_letter.clone(),
                member.file_size.to_string(),
                member.last_modified_unix_nanos.clone(),
            ]));
        }
    }
    csv
}

const SESSION_CSV_HEADER: &[&str] = &[
    "sessionId",
    "sessionName",
    "roots",
    "ignorePatterns",
    "cloudPolicy",
    "sessionCreatedAt",
    "sessionUpdatedAt",
    "runId",
    "runStatus",
    "runCreatedAt",
    "runStartedAt",
    "runCompletedAt",
    "filesDiscovered",
    "bytesDiscovered",
    "filesHashed",
    "duplicateFileGroups",
    "duplicateFolderGroups",
    "wastedBytes",
    "warningCount",
    "excludedSubtreeCount",
    "engineVersion",
];

/// Joins a multi-value field's entries with `;` for one CSV cell; see `docs/export-format-v1.md`.
const CSV_LIST_SEPARATOR: &str = ";";

fn sessions_csv(document: &SessionsExport) -> String {
    let mut csv = csv_row(SESSION_CSV_HEADER);
    for session in &document.sessions {
        let roots = session.roots.join(CSV_LIST_SEPARATOR);
        let ignore_patterns = session.ignore_patterns.join(CSV_LIST_SEPARATOR);
        if session.runs.is_empty() {
            csv.push_str(&csv_row(&[
                session.id.to_string(),
                session.name.clone(),
                roots.clone(),
                ignore_patterns.clone(),
                session.cloud_policy.clone(),
                session.created_at.clone(),
                session.updated_at.clone(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ]));
            continue;
        }
        for run in &session.runs {
            csv.push_str(&csv_row(&[
                session.id.to_string(),
                session.name.clone(),
                roots.clone(),
                ignore_patterns.clone(),
                session.cloud_policy.clone(),
                session.created_at.clone(),
                session.updated_at.clone(),
                run.id.to_string(),
                run.status.clone(),
                run.created_at.clone(),
                run.started_at.clone().unwrap_or_default(),
                run.completed_at.clone().unwrap_or_default(),
                run.files_discovered.to_string(),
                run.bytes_discovered.to_string(),
                run.files_hashed.to_string(),
                run.duplicate_file_groups.to_string(),
                run.duplicate_folder_groups.to_string(),
                run.wasted_bytes.to_string(),
                run.warning_count.to_string(),
                run.excluded_subtree_count.to_string(),
                run.engine_version.clone(),
            ]));
        }
    }
    csv
}

/// One RFC 4180 row: comma-separated, CRLF-terminated, quoting only fields that need it.
fn csv_row(fields: &[impl AsRef<str>]) -> String {
    let mut row = fields
        .iter()
        .map(|field| csv_field(field.as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    row.push_str("\r\n");
    row
}

fn csv_field(value: &str) -> String {
    if value.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use crate::storage::models::RunParameters;

    fn fixture_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let params = RunParameters {
            roots: vec!["D:\\Photos".to_owned()],
            ignore_patterns: vec!["**/node_modules/**".to_owned()],
            directory_similarity_threshold_millis: 500,
            repeat_cache_policy: Default::default(),
            cloud_policy: Default::default(),
            manual_location_exclusions: Vec::new(),
            registered_cloud_locations: Vec::new(),
            cloud_detection_status: Default::default(),
        };
        let session_id = db
            .create_session("Photos", &params.roots, &params.ignore_patterns)
            .unwrap();
        let run_id = db.create_scan_run(session_id, &params, "test").unwrap();
        db.start_scan_run(run_id).unwrap();
        db.complete_scan_run(run_id, 2, 2048, 2, 1, 0, 1024, 0)
            .unwrap();
        let files = vec![
            crate::storage::models::ScannedFile {
                id: 0,
                run_id,
                root_path: "D:\\Photos".to_owned(),
                canonical_path: r"\\?\D:\Photos\a.jpg".to_owned(),
                relative_path: "a.jpg".to_owned(),
                file_name: "a.jpg".to_owned(),
                parent_dir: r"\\?\D:\Photos".to_owned(),
                drive_letter: "D:".to_owned(),
                file_size: 1024,
                last_modified: 1_786_795_200_000_000_000,
                partial_hash: Some(1),
                content_hash: Some(42),
                file_identity: None,
                warning_message: None,
                marked_deleted: false,
            },
            crate::storage::models::ScannedFile {
                id: 0,
                run_id,
                root_path: "D:\\Photos".to_owned(),
                canonical_path: r"\\?\D:\Photos\Copy\a.jpg".to_owned(),
                relative_path: "Copy\\a.jpg".to_owned(),
                file_name: "a.jpg".to_owned(),
                parent_dir: r"\\?\D:\Photos\Copy".to_owned(),
                drive_letter: "D:".to_owned(),
                file_size: 1024,
                last_modified: 1_786_795_200_000_000_000,
                partial_hash: Some(1),
                content_hash: Some(42),
                file_identity: None,
                warning_message: None,
                marked_deleted: false,
            },
        ];
        db.insert_scanned_files(&files).unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[(
                42,
                1024,
                vec![
                    r"\\?\D:\Photos\a.jpg".to_owned(),
                    r"\\?\D:\Photos\Copy\a.jpg".to_owned(),
                ],
            )],
        )
        .unwrap();
        db
    }

    #[test]
    fn duplicate_groups_json_uses_plain_paths_and_stringifies_wide_integers() {
        let db = fixture_db();
        let json = export_duplicate_groups(&db, None, ExportFormat::Json).unwrap();
        assert!(!json.contains(r"\\?\"), "verbatim prefix leaked: {json}");
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["formatVersion"], 1);
        let group = &value["groups"][0];
        assert_eq!(group["contentHash"], "000000000000002a");
        let paths: Vec<&str> = group["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| member["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains(&"D:\\Photos\\a.jpg"));
        assert!(paths.contains(&"D:\\Photos\\Copy\\a.jpg"));
        assert_eq!(
            group["members"][0]["lastModifiedUnixNanos"],
            "1786795200000000000"
        );
    }

    #[test]
    fn duplicate_groups_csv_has_one_row_per_member_and_a_stable_header() {
        let db = fixture_db();
        let csv = export_duplicate_groups(&db, None, ExportFormat::Csv).unwrap();
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap(), DUPLICATE_GROUP_CSV_HEADER.join(","));
        let rows: Vec<&str> = lines.collect();
        assert_eq!(rows.len(), 2, "one row per member: {csv}");
        for row in &rows {
            assert!(row.starts_with("1,000000000000002a,1024,2,1024,"), "{row}");
        }
        assert!(csv.contains("D:\\Photos\\a.jpg,"));
        assert!(csv.contains("D:\\Photos\\Copy\\a.jpg,"));
    }

    #[test]
    fn sessions_export_round_trips_roots_and_run_history() {
        let db = fixture_db();
        let json = export_sessions(&db, None, ExportFormat::Json).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let session = &value["sessions"][0];
        assert_eq!(session["name"], "Photos");
        assert_eq!(session["roots"], serde_json::json!(["D:\\Photos"]));
        assert_eq!(session["runs"][0]["filesDiscovered"], 2);

        let csv = export_sessions(&db, None, ExportFormat::Csv).unwrap();
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap(), SESSION_CSV_HEADER.join(","));
        let row = lines.next().unwrap();
        assert!(
            row.starts_with("1,Photos,D:\\Photos,**/node_modules/**,exclude_registered_roots,")
        );
    }

    #[test]
    fn sessions_export_by_id_rejects_an_unknown_session() {
        let db = fixture_db();
        let error = export_sessions(&db, Some(999), ExportFormat::Json).unwrap_err();
        assert!(error.to_string().contains("session 999 not found"));
    }

    #[test]
    fn duplicate_groups_export_rejects_an_unknown_run() {
        let db = fixture_db();
        let error = export_duplicate_groups(&db, Some(999), ExportFormat::Json).unwrap_err();
        assert!(error.to_string().contains("run 999 not found"));
    }

    #[test]
    fn duplicate_groups_export_without_a_completed_run_is_a_clear_error() {
        let db = Database::open_in_memory().unwrap();
        let error = export_duplicate_groups(&db, None, ExportFormat::Json).unwrap_err();
        assert!(error.to_string().contains("no completed run is available"));
    }

    #[test]
    fn csv_field_quotes_only_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_field("a\nb"), "\"a\nb\"");
    }
}

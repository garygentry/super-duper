use std::collections::HashSet;

use rusqlite::{params, params_from_iter, types::Value, OptionalExtension};
use thiserror::Error;

use super::live_validation::{normalize_path, path_is_within};
use super::models::{
    ReviewLiveHintRequest, ReviewLiveHintResult, ReviewLiveHintTarget, RunParameters,
};
use super::Database;

const MAXIMUM_HINT_PATHS: usize = 200;

#[derive(Debug, Error)]
pub enum ReviewLiveHintError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid live hint request: {message}")]
    InvalidRequest { message: String },
    #[error("scan run {run_id} was not found")]
    RunNotFound { run_id: i64 },
    #[error("scan run {run_id} is {status}; live hints require a completed run")]
    RunNotCompleted { run_id: i64, status: String },
    #[error("root {root_path} is not an immutable selected root for run {run_id}")]
    RootNotFound { run_id: i64, root_path: String },
    #[error("run {run_id} has an invalid parameter snapshot")]
    InvalidRunParameters { run_id: i64 },
}

impl Database {
    pub fn resolve_review_live_hints(
        &self,
        request: &ReviewLiveHintRequest,
    ) -> Result<ReviewLiveHintResult, ReviewLiveHintError> {
        validate_request(request)?;
        let (status, parameters_json) = self
            .connection()
            .query_row(
                "SELECT status, parameters_json FROM scan_run WHERE id = ?1",
                params![request.run_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(ReviewLiveHintError::RunNotFound {
                run_id: request.run_id,
            })?;
        if status != "completed" {
            return Err(ReviewLiveHintError::RunNotCompleted {
                run_id: request.run_id,
                status,
            });
        }
        let parameters = RunParameters::from_json(&parameters_json).ok_or(
            ReviewLiveHintError::InvalidRunParameters {
                run_id: request.run_id,
            },
        )?;
        let normalized_root = normalize_path(&request.root_path);
        let root_path = parameters
            .roots
            .into_iter()
            .find(|root| normalize_path(root) == normalized_root)
            .ok_or_else(|| ReviewLiveHintError::RootNotFound {
                run_id: request.run_id,
                root_path: request.root_path.clone(),
            })?;
        if request
            .paths
            .iter()
            .any(|path| !path_is_within(path, &root_path))
        {
            return Err(ReviewLiveHintError::InvalidRequest {
                message: "every path must be within the exact immutable selected root".to_owned(),
            });
        }

        let placeholders = std::iter::repeat("?")
            .take(request.paths.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT file.id, member.group_id, file.canonical_path
             FROM scanned_file file
             JOIN duplicate_group_member member ON member.file_id = file.id
             JOIN duplicate_group duplicate_set
               ON duplicate_set.id = member.group_id AND duplicate_set.run_id = file.run_id
             WHERE file.run_id = ? AND file.canonical_path COLLATE UNICODE_NOCASE IN ({placeholders})
             ORDER BY file.id
             LIMIT 200"
        );
        let mut values = Vec::with_capacity(request.paths.len() + 1);
        values.push(Value::Integer(request.run_id));
        values.extend(request.paths.iter().cloned().map(Value::Text));
        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            Ok(ReviewLiveHintTarget {
                file_id: row.get(0)?,
                group_id: row.get(1)?,
                path: row.get(2)?,
            })
        })?;
        Ok(ReviewLiveHintResult {
            run_id: request.run_id,
            root_path,
            event_count: request.event_count,
            coalesced_path_count: request.paths.len() as i64,
            items: rows.collect::<Result<Vec<_>, _>>()?,
        })
    }
}

fn validate_request(request: &ReviewLiveHintRequest) -> Result<(), ReviewLiveHintError> {
    let distinct = request
        .paths
        .iter()
        .map(|path| normalize_path(path))
        .collect::<HashSet<_>>();
    if request.run_id <= 0
        || request.root_path.trim().is_empty()
        || request.root_path.chars().count() > 32_767
        || request.event_count <= 0
        || request.event_count < request.paths.len() as i64
        || request.paths.is_empty()
        || request.paths.len() > MAXIMUM_HINT_PATHS
        || request
            .paths
            .iter()
            .any(|path| path.trim().is_empty() || path.chars().count() > 32_767)
        || distinct.len() != request.paths.len()
    {
        return Err(ReviewLiveHintError::InvalidRequest {
            message: "runId/eventCount must be positive; rootPath and each path must contain 1..=32767 characters; eventCount must cover 1..=200 distinct paths".to_owned(),
        });
    }
    Ok(())
}

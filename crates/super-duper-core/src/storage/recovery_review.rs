use std::hash::Hasher;

use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, OptionalExtension, Row, Transaction, TransactionBehavior};
use thiserror::Error;
use twox_hash::XxHash64;

use super::models::{
    RecoveryObservationKind, RecoveryReviewMutationResult, RecoveryReviewObservation,
    RecoveryReviewObservationInput, RecoveryReviewObservationPage, RecoveryReviewState,
    RecoveryReviewSummary,
};
use super::Database;

const MAXIMUM_REQUEST_ID_CHARACTERS: usize = 128;
const MAXIMUM_NOTE_CHARACTERS: usize = 1_000;
const MAXIMUM_CORRECTION_REASON_CHARACTERS: usize = 500;
const MAXIMUM_TIMESTAMP_CHARACTERS: usize = 64;
const CURRENT_EVIDENCE_VERSION: i64 = 1;

#[derive(Debug, Error)]
pub enum RecoveryReviewError {
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error("invalid recovery-review request: {message}")]
    InvalidRequest { message: String },
    #[error("recycle operation {operation_id} was not found")]
    OperationNotFound { operation_id: i64 },
    #[error("recycle operation {operation_id} cannot be reviewed from {status}")]
    InvalidOperationState { operation_id: i64, status: String },
    #[error("item {item_id} does not belong to recycle operation {operation_id}")]
    ItemNotFound { operation_id: i64, item_id: i64 },
    #[error("item {item_id} in recycle operation {operation_id} is {result_status}, not unknown")]
    NonUnknownItem {
        operation_id: i64,
        item_id: i64,
        result_status: String,
    },
    #[error("recovery observation {observation_id} was not found")]
    ObservationNotFound { observation_id: i64 },
    #[error(
        "recovery observation {observation_id} is not the current observation for item {item_id}"
    )]
    SupersessionConflict { observation_id: i64, item_id: i64 },
    #[error("item {item_id} already has current recovery observation {observation_id}")]
    CurrentObservationExists { item_id: i64, observation_id: i64 },
    #[error("request id {request_id} was already used with another recovery-review payload")]
    IdempotencyConflict { request_id: String },
}

impl Database {
    pub fn get_recovery_review(
        &self,
        operation_id: i64,
    ) -> Result<RecoveryReviewSummary, RecoveryReviewError> {
        validate_reviewable_operation(self.connection(), operation_id)?;
        recovery_review_summary(self.connection(), operation_id)
    }

    pub fn page_recovery_review_observations(
        &self,
        operation_id: i64,
        offset: i64,
        limit: i64,
        current_only: bool,
    ) -> Result<RecoveryReviewObservationPage, RecoveryReviewError> {
        if offset < 0 || !(1..=200).contains(&limit) {
            return Err(RecoveryReviewError::InvalidRequest {
                message: "offset must be non-negative and limit must be between 1 and 200"
                    .to_owned(),
            });
        }
        validate_reviewable_operation(self.connection(), operation_id)?;
        let total = self.connection().query_row(
            "SELECT COUNT(*)
             FROM recovery_review_observation observation
             WHERE observation.recycle_operation_id = ?1
               AND (NOT ?2 OR NOT EXISTS (
                    SELECT 1 FROM recovery_review_observation successor
                    WHERE successor.supersedes_observation_id = observation.id
               ))",
            params![operation_id, current_only],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = self.connection().prepare(
            "SELECT observation.id, observation.request_id,
                    observation.recycle_operation_id, observation.item_id,
                    observation.observation, observation.observed_at, observation.note,
                    observation.evidence_version, observation.supersedes_observation_id,
                    observation.correction_reason, observation.created_at,
                    successor.id, successor.id IS NULL
             FROM recovery_review_observation observation
             LEFT JOIN recovery_review_observation successor
               ON successor.supersedes_observation_id = observation.id
             WHERE observation.recycle_operation_id = ?1
               AND (NOT ?2 OR successor.id IS NULL)
             ORDER BY observation.id
             LIMIT ?3 OFFSET ?4",
        )?;
        let observations = statement
            .query_map(
                params![operation_id, current_only, limit, offset],
                recovery_observation_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RecoveryReviewObservationPage {
            has_more: offset + (observations.len() as i64) < total,
            observations,
            total,
        })
    }

    pub fn record_recovery_review_observation(
        &self,
        input: &RecoveryReviewObservationInput,
    ) -> Result<RecoveryReviewMutationResult, RecoveryReviewError> {
        validate_input(input)?;
        let signature = payload_signature(input);
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;

        if let Some((observation_id, existing_signature)) = tx
            .query_row(
                "SELECT id, payload_signature FROM recovery_review_observation
                 WHERE request_id = ?1",
                params![input.request_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if existing_signature != signature {
                return Err(RecoveryReviewError::IdempotencyConflict {
                    request_id: input.request_id.clone(),
                });
            }
            tx.commit()?;
            return Ok(RecoveryReviewMutationResult {
                summary: self.get_recovery_review(input.recycle_operation_id)?,
                observation: recovery_observation_by_id(self.connection(), observation_id)?
                    .ok_or(RecoveryReviewError::ObservationNotFound { observation_id })?,
                replayed: true,
            });
        }

        validate_reviewable_operation(&tx, input.recycle_operation_id)?;
        let result_status = tx
            .query_row(
                "SELECT result_status FROM recycle_operation_item
                 WHERE id = ?1 AND recycle_operation_id = ?2",
                params![input.item_id, input.recycle_operation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(RecoveryReviewError::ItemNotFound {
                operation_id: input.recycle_operation_id,
                item_id: input.item_id,
            })?;
        if result_status != "unknown" {
            return Err(RecoveryReviewError::NonUnknownItem {
                operation_id: input.recycle_operation_id,
                item_id: input.item_id,
                result_status,
            });
        }

        let current_observation = current_observation_id(&tx, input.item_id)?;
        match input.supersedes_observation_id {
            Some(prior_id) => {
                let prior = tx
                    .query_row(
                        "SELECT recycle_operation_id, item_id,
                                NOT EXISTS (
                                    SELECT 1 FROM recovery_review_observation successor
                                    WHERE successor.supersedes_observation_id = observation.id
                                )
                         FROM recovery_review_observation observation WHERE id = ?1",
                        params![prior_id],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, bool>(2)?,
                            ))
                        },
                    )
                    .optional()?
                    .ok_or(RecoveryReviewError::ObservationNotFound {
                        observation_id: prior_id,
                    })?;
                if prior.0 != input.recycle_operation_id
                    || prior.1 != input.item_id
                    || !prior.2
                    || current_observation != Some(prior_id)
                {
                    return Err(RecoveryReviewError::SupersessionConflict {
                        observation_id: prior_id,
                        item_id: input.item_id,
                    });
                }
            }
            None => {
                if let Some(observation_id) = current_observation {
                    return Err(RecoveryReviewError::CurrentObservationExists {
                        item_id: input.item_id,
                        observation_id,
                    });
                }
            }
        }

        tx.execute(
            "INSERT INTO recovery_review_observation
                (request_id, payload_signature, recycle_operation_id, item_id, observation,
                 observed_at, note, evidence_version, supersedes_observation_id,
                 correction_reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                input.request_id,
                signature,
                input.recycle_operation_id,
                input.item_id,
                input.observation.as_str(),
                input.observed_at,
                input.note,
                input.evidence_version,
                input.supersedes_observation_id,
                input.correction_reason,
                Utc::now().to_rfc3339(),
            ],
        )?;
        let observation_id = tx.last_insert_rowid();
        tx.commit()?;

        Ok(RecoveryReviewMutationResult {
            summary: self.get_recovery_review(input.recycle_operation_id)?,
            observation: recovery_observation_by_id(self.connection(), observation_id)?
                .ok_or(RecoveryReviewError::ObservationNotFound { observation_id })?,
            replayed: false,
        })
    }
}

fn validate_input(input: &RecoveryReviewObservationInput) -> Result<(), RecoveryReviewError> {
    if input.request_id.is_empty()
        || input.request_id.chars().count() > MAXIMUM_REQUEST_ID_CHARACTERS
    {
        return Err(RecoveryReviewError::InvalidRequest {
            message: "requestId must contain 1 to 128 characters".to_owned(),
        });
    }
    if input.recycle_operation_id <= 0 || input.item_id <= 0 {
        return Err(RecoveryReviewError::InvalidRequest {
            message: "recycleOperationId and itemId must be positive".to_owned(),
        });
    }
    if input.evidence_version != CURRENT_EVIDENCE_VERSION {
        return Err(RecoveryReviewError::InvalidRequest {
            message: "evidenceVersion must be 1".to_owned(),
        });
    }
    if input.observed_at.is_empty()
        || input.observed_at.chars().count() > MAXIMUM_TIMESTAMP_CHARACTERS
        || DateTime::parse_from_rfc3339(&input.observed_at).is_err()
    {
        return Err(RecoveryReviewError::InvalidRequest {
            message: "observedAt must be an RFC 3339 timestamp of at most 64 characters".to_owned(),
        });
    }
    if input
        .note
        .as_ref()
        .is_some_and(|value| value.chars().count() > MAXIMUM_NOTE_CHARACTERS)
    {
        return Err(RecoveryReviewError::InvalidRequest {
            message: "note must contain at most 1000 characters".to_owned(),
        });
    }
    match (
        input.supersedes_observation_id,
        input.correction_reason.as_ref(),
    ) {
        (None, None) => {}
        (Some(id), Some(reason))
            if id > 0
                && !reason.is_empty()
                && reason.chars().count() <= MAXIMUM_CORRECTION_REASON_CHARACTERS => {}
        _ => {
            return Err(RecoveryReviewError::InvalidRequest {
                message: "supersession requires a positive prior observation ID and a correction reason of 1 to 500 characters; both must otherwise be omitted".to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_reviewable_operation(
    connection: &rusqlite::Connection,
    operation_id: i64,
) -> Result<(), RecoveryReviewError> {
    if operation_id <= 0 {
        return Err(RecoveryReviewError::InvalidRequest {
            message: "recycleOperationId must be positive".to_owned(),
        });
    }
    let status = connection
        .query_row(
            "SELECT status FROM recycle_operation WHERE id = ?1",
            params![operation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(RecoveryReviewError::OperationNotFound { operation_id })?;
    if status != "recovery_required" {
        return Err(RecoveryReviewError::InvalidOperationState {
            operation_id,
            status,
        });
    }
    Ok(())
}

fn recovery_review_summary(
    connection: &rusqlite::Connection,
    operation_id: i64,
) -> Result<RecoveryReviewSummary, RecoveryReviewError> {
    let (unknown_item_count, observed_item_count) = connection.query_row(
        "SELECT
             COUNT(*),
             COALESCE(SUM(CASE WHEN EXISTS (
                 SELECT 1 FROM recovery_review_observation observation
                 WHERE observation.item_id = item.id
                   AND observation.recycle_operation_id = item.recycle_operation_id
                   AND NOT EXISTS (
                       SELECT 1 FROM recovery_review_observation successor
                       WHERE successor.supersedes_observation_id = observation.id
                   )
             ) THEN 1 ELSE 0 END), 0)
         FROM recycle_operation_item item
         WHERE item.recycle_operation_id = ?1 AND item.result_status = 'unknown'",
        params![operation_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let state = if observed_item_count == 0 {
        RecoveryReviewState::NotStarted
    } else if observed_item_count < unknown_item_count {
        RecoveryReviewState::InProgress
    } else {
        RecoveryReviewState::ReviewCompleteWithUnresolvedEvidence
    };
    Ok(RecoveryReviewSummary {
        recycle_operation_id: operation_id,
        state,
        unknown_item_count,
        observed_item_count,
    })
}

fn current_observation_id(
    connection: &rusqlite::Connection,
    item_id: i64,
) -> rusqlite::Result<Option<i64>> {
    connection
        .query_row(
            "SELECT observation.id FROM recovery_review_observation observation
             WHERE observation.item_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM recovery_review_observation successor
                   WHERE successor.supersedes_observation_id = observation.id
               )",
            params![item_id],
            |row| row.get(0),
        )
        .optional()
}

fn recovery_observation_by_id(
    connection: &rusqlite::Connection,
    observation_id: i64,
) -> rusqlite::Result<Option<RecoveryReviewObservation>> {
    connection
        .query_row(
            "SELECT observation.id, observation.request_id,
                    observation.recycle_operation_id, observation.item_id,
                    observation.observation, observation.observed_at, observation.note,
                    observation.evidence_version, observation.supersedes_observation_id,
                    observation.correction_reason, observation.created_at,
                    successor.id, successor.id IS NULL
             FROM recovery_review_observation observation
             LEFT JOIN recovery_review_observation successor
               ON successor.supersedes_observation_id = observation.id
             WHERE observation.id = ?1",
            params![observation_id],
            recovery_observation_from_row,
        )
        .optional()
}

fn recovery_observation_from_row(row: &Row<'_>) -> rusqlite::Result<RecoveryReviewObservation> {
    let value = row.get::<_, String>(4)?;
    let observation = RecoveryObservationKind::parse(&value).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(4, "observation".to_owned(), Type::Text)
    })?;
    Ok(RecoveryReviewObservation {
        id: row.get(0)?,
        request_id: row.get(1)?,
        recycle_operation_id: row.get(2)?,
        item_id: row.get(3)?,
        observation,
        observed_at: row.get(5)?,
        note: row.get(6)?,
        evidence_version: row.get(7)?,
        supersedes_observation_id: row.get(8)?,
        correction_reason: row.get(9)?,
        created_at: row.get(10)?,
        superseded_by_observation_id: row.get(11)?,
        is_current: row.get(12)?,
    })
}

fn payload_signature(input: &RecoveryReviewObservationInput) -> String {
    let mut hasher = XxHash64::with_seed(0x7265636f76657279);
    hasher.write_i64(input.recycle_operation_id);
    hasher.write_i64(input.item_id);
    write_text(&mut hasher, input.observation.as_str());
    write_text(&mut hasher, &input.observed_at);
    write_optional_text(&mut hasher, input.note.as_deref());
    hasher.write_i64(input.evidence_version);
    hasher.write_i64(input.supersedes_observation_id.unwrap_or(-1));
    write_optional_text(&mut hasher, input.correction_reason.as_deref());
    format!("{:016x}", hasher.finish())
}

fn write_text(hasher: &mut XxHash64, value: &str) {
    hasher.write_u64(value.len() as u64);
    hasher.write(value.as_bytes());
}

fn write_optional_text(hasher: &mut XxHash64, value: Option<&str>) {
    hasher.write_u8(value.is_some() as u8);
    if let Some(value) = value {
        write_text(hasher, value);
    }
}

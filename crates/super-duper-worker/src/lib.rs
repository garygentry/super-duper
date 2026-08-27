use glob::Pattern;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fmt;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use super_duper_core::progress::ProgressReporter;
use super_duper_core::storage::live_hints::ReviewLiveHintError;
use super_duper_core::storage::live_validation::ReviewLiveValidationError;
use super_duper_core::storage::models::{
    CloudDetectionStatus, CloudPolicy, DuplicateFileDriveFacetPageQuery,
    DuplicateFileDriveFacetResult, DuplicateFileDriveFacetSortField,
    DuplicateFileExtensionMatchMode, DuplicateFileGroupFilter, DuplicateFileGroupPageQuery,
    DuplicateFileGroupResult, DuplicateFileGroupSortField, DuplicateFileMemberFilter,
    DuplicateFileMemberPageQuery, DuplicateFileMemberResult, DuplicateFileMemberSortField,
    DuplicateFilePathMatchMode, DuplicateFileSelectedRootFacetPageQuery,
    DuplicateFileSelectedRootFacetResult, DuplicateFileSelectedRootFacetSortField,
    DuplicateFolderGroupFilter, DuplicateFolderGroupPageQuery, DuplicateFolderGroupResult,
    DuplicateFolderGroupSortField, DuplicateFolderMemberFilter, DuplicateFolderMemberPageQuery,
    DuplicateFolderMemberResult, DuplicateFolderMemberSortField, PageCursor, PageCursorValue,
    PreferencePreviewGroup, PreferencePreviewScope, PreferencePreviewSummary, PreferenceRule,
    PreferenceRuleApplication, PreferenceRuleSummary, Preflight, PreflightItem, PreflightView,
    RecoveryObservationKind, RecoveryReviewObservation, RecoveryReviewObservationInput,
    RecoveryReviewSummary, RecycleEligibilityObservation, RecycleItemResultObservation,
    RecycleOperation, RecycleOperationBatch, RecycleOperationItem, RecycleOperationView,
    RegisteredCloudLocation, ReviewDecisionKind, ReviewFolderGroupSummary, ReviewGroupSummary,
    ReviewLiveHintRequest, ReviewLiveRootOverflowRequest, ReviewLiveRootReconciliationRequest,
    ReviewLiveRootState, ReviewLiveValidationRequest, ReviewPlanSummary, ReviewPlanView,
    RunExclusion, RunParameters, RunWarningAggregate, RunWarningPageQuery, RunWarningSortField,
    ScanRun, ScanSession, SortDirection,
};
use super_duper_core::storage::preference::PreferenceError;
use super_duper_core::storage::preflight::PreflightError;
use super_duper_core::storage::recovery_review::RecoveryReviewError;
use super_duper_core::storage::recycle_operation::RecycleOperationError;
use super_duper_core::storage::review::ReviewError;
use super_duper_core::storage::root_reconciliation::ReviewLiveRootError;
use super_duper_core::storage::Database;
use super_duper_core::telemetry::{
    ProgressObservation, ProgressReducer, StatusDatabase, TelemetryPhase,
};
use super_duper_core::{AppConfig, ScanEngine};

mod progress_projection;

use progress_projection::{
    progress_event_data, LatestValueCoalescer, LegacyProgressProjection, PendingProgress,
};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAXIMUM_FRAME_BYTES: usize = 1_048_576;
const DEFAULT_PAGE_SIZE: i64 = 100;
const DEFAULT_RESULT_PAGE_SIZE: i64 = 200;
const MAXIMUM_PAGE_SIZE: i64 = 500;
const MAXIMUM_FILTER_CHARACTERS: usize = 512;
const MAXIMUM_EXACT_PATH_CHARACTERS: usize = 32_767;
const MAXIMUM_EXTENSION_CHARACTERS: usize = 255;
const MAXIMUM_CURSOR_CHARACTERS: usize = MAXIMUM_FRAME_BYTES / 2;
const MAXIMUM_OPERATION_ID_CHARACTERS: usize = 128;
const EVENT_INTERVAL: Duration = Duration::from_millis(100);
const DATABASE_PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
const PERFORMANCE_HISTORY_PAGE_SIZE: i64 = 25;

#[derive(Debug)]
pub enum WorkerError {
    Io(io::Error),
    FatalProtocol(String),
    Startup(String),
}

impl fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::FatalProtocol(message) | Self::Startup(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WorkerError {}

impl From<io::Error> for WorkerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone)]
pub struct WorkerOptions {
    pub database_path: PathBuf,
    pub status_database_path: PathBuf,
    pub diagnostic_log_path: Option<PathBuf>,
}

impl WorkerOptions {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        let database_path = database_path.into();
        let status_database_path = database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("scan_status.db");
        Self {
            database_path,
            status_database_path,
            diagnostic_log_path: None,
        }
    }

    pub fn with_status_database_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.status_database_path = path.into();
        self
    }

    pub fn with_diagnostic_log_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.diagnostic_log_path = Some(path.into());
        self
    }
}

impl Default for WorkerOptions {
    fn default() -> Self {
        let options = Self::new(
            std::env::var_os("SUPER_DUPER_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("super_duper.db")),
        );
        let options = match std::env::var_os("SUPER_DUPER_STATUS_DB_PATH") {
            Some(path) => options.with_status_database_path(PathBuf::from(path)),
            None => options,
        };
        match std::env::var_os("SUPER_DUPER_DIAGNOSTIC_LOG_PATH") {
            Some(path) => options.with_diagnostic_log_path(PathBuf::from(path)),
            None => options,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RequestEnvelope {
    #[serde(rename = "type")]
    message_type: String,
    id: String,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelloParameters {
    protocol_versions: Vec<u32>,
    client: ClientInformation,
}

#[derive(Debug, Deserialize)]
struct ClientInformation {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct ResponseEnvelope {
    #[serde(rename = "type")]
    message_type: &'static str,
    id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<StructuredError>,
}

#[derive(Debug, Serialize)]
struct StructuredError {
    code: String,
    message: String,
    retryable: bool,
    details: Value,
}

#[derive(Debug)]
struct ProtocolFailure {
    code: &'static str,
    message: String,
    retryable: bool,
    details: Value,
}

impl ProtocolFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            details: json!({}),
        }
    }

    fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageParameters {
    #[serde(default)]
    offset: i64,
    #[serde(default = "default_page_size")]
    limit: i64,
}

impl Default for PageParameters {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdParameters {
    #[serde(alias = "id")]
    session_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunIdParameters {
    #[serde(alias = "id")]
    run_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunExclusionPageParameters {
    run_id: i64,
    #[serde(default)]
    offset: i64,
    #[serde(default = "default_page_size")]
    limit: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WarningPageParameters {
    run_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    sort: WarningSortParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WarningSortParameters {
    #[serde(default = "default_warning_sort_field")]
    field: String,
    #[serde(default = "default_descending")]
    direction: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PerformanceRunPageParameters {
    #[serde(default)]
    before_id: Option<i64>,
    #[serde(default = "default_performance_history_page_size")]
    page_size: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PerformanceSnapshotParameters {
    #[serde(default)]
    status_run_id: Option<i64>,
    #[serde(default)]
    product_run_id: Option<i64>,
}

impl Default for WarningSortParameters {
    fn default() -> Self {
        Self {
            field: default_warning_sort_field(),
            direction: default_descending(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionWriteParameters {
    name: String,
    roots: Vec<String>,
    #[serde(default)]
    ignore_patterns: Vec<String>,
    #[serde(default)]
    cloud_policy: CloudPolicy,
    #[serde(default)]
    manual_location_exclusions: Vec<String>,
    #[serde(default)]
    registered_cloud_locations: Vec<RegisteredCloudLocation>,
    #[serde(default)]
    cloud_detection_status: CloudDetectionStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionUpdateParameters {
    session_id: i64,
    name: String,
    roots: Vec<String>,
    #[serde(default)]
    ignore_patterns: Vec<String>,
    #[serde(default)]
    cloud_policy: CloudPolicy,
    #[serde(default)]
    manual_location_exclusions: Vec<String>,
    #[serde(default)]
    registered_cloud_locations: Vec<RegisteredCloudLocation>,
    #[serde(default)]
    cloud_detection_status: CloudDetectionStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunListParameters {
    #[serde(default)]
    session_id: Option<i64>,
    #[serde(default)]
    offset: i64,
    #[serde(default = "default_page_size")]
    limit: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileGroupPageParameters {
    run_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    sort: DuplicateFileGroupSortParameters,
    #[serde(default)]
    filter: DuplicateFileGroupFilterParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileGroupSortParameters {
    #[serde(default = "default_group_sort_field")]
    field: String,
    #[serde(default = "default_descending")]
    direction: String,
}

impl Default for DuplicateFileGroupSortParameters {
    fn default() -> Self {
        Self {
            field: default_group_sort_field(),
            direction: default_descending(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileGroupFilterParameters {
    #[serde(default)]
    search: Option<String>,
    #[serde(default = "default_path_match")]
    path_match: String,
    #[serde(default)]
    extension: Option<String>,
    #[serde(default = "default_extension_match")]
    extension_match: String,
    #[serde(default = "default_minimum_size")]
    minimum_size: String,
    #[serde(default = "default_minimum_copy_count")]
    minimum_copy_count: i64,
    #[serde(default)]
    across_drives: bool,
    #[serde(default)]
    selected_root: Option<String>,
    #[serde(default)]
    selected_drive: Option<String>,
}

impl Default for DuplicateFileGroupFilterParameters {
    fn default() -> Self {
        Self {
            search: None,
            path_match: default_path_match(),
            extension: None,
            extension_match: default_extension_match(),
            minimum_size: default_minimum_size(),
            minimum_copy_count: default_minimum_copy_count(),
            across_drives: false,
            selected_root: None,
            selected_drive: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileSelectedRootFacetPageParameters {
    run_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    sort: DuplicateFileSelectedRootFacetSortParameters,
    #[serde(default)]
    filter: DuplicateFileSelectedRootFacetFilterParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileSelectedRootFacetSortParameters {
    #[serde(default = "default_selected_root_facet_sort_field")]
    field: String,
    #[serde(default = "default_descending")]
    direction: String,
}

impl Default for DuplicateFileSelectedRootFacetSortParameters {
    fn default() -> Self {
        Self {
            field: default_selected_root_facet_sort_field(),
            direction: default_descending(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileSelectedRootFacetFilterParameters {
    #[serde(default)]
    search: Option<String>,
    #[serde(default = "default_path_match")]
    path_match: String,
    #[serde(default)]
    extension: Option<String>,
    #[serde(default = "default_extension_match")]
    extension_match: String,
    #[serde(default = "default_minimum_size")]
    minimum_size: String,
    #[serde(default = "default_minimum_copy_count")]
    minimum_copy_count: i64,
    #[serde(default)]
    across_drives: bool,
    #[serde(default)]
    selected_drive: Option<String>,
}

impl Default for DuplicateFileSelectedRootFacetFilterParameters {
    fn default() -> Self {
        Self {
            search: None,
            path_match: default_path_match(),
            extension: None,
            extension_match: default_extension_match(),
            minimum_size: default_minimum_size(),
            minimum_copy_count: default_minimum_copy_count(),
            across_drives: false,
            selected_drive: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileDriveFacetPageParameters {
    run_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    sort: DuplicateFileDriveFacetSortParameters,
    #[serde(default)]
    filter: DuplicateFileDriveFacetFilterParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileDriveFacetSortParameters {
    #[serde(default = "default_drive_facet_sort_field")]
    field: String,
    #[serde(default = "default_descending")]
    direction: String,
}

impl Default for DuplicateFileDriveFacetSortParameters {
    fn default() -> Self {
        Self {
            field: default_drive_facet_sort_field(),
            direction: default_descending(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileDriveFacetFilterParameters {
    #[serde(default)]
    search: Option<String>,
    #[serde(default = "default_path_match")]
    path_match: String,
    #[serde(default)]
    extension: Option<String>,
    #[serde(default = "default_extension_match")]
    extension_match: String,
    #[serde(default = "default_minimum_size")]
    minimum_size: String,
    #[serde(default = "default_minimum_copy_count")]
    minimum_copy_count: i64,
    #[serde(default)]
    across_drives: bool,
    #[serde(default)]
    selected_root: Option<String>,
}

impl Default for DuplicateFileDriveFacetFilterParameters {
    fn default() -> Self {
        Self {
            search: None,
            path_match: default_path_match(),
            extension: None,
            extension_match: default_extension_match(),
            minimum_size: default_minimum_size(),
            minimum_copy_count: default_minimum_copy_count(),
            across_drives: false,
            selected_root: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileMemberPageParameters {
    run_id: i64,
    group_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    sort: DuplicateFileMemberSortParameters,
    #[serde(default)]
    filter: DuplicateFileMemberFilterParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileMemberSortParameters {
    #[serde(default = "default_member_sort_field")]
    field: String,
    #[serde(default = "default_ascending")]
    direction: String,
}

impl Default for DuplicateFileMemberSortParameters {
    fn default() -> Self {
        Self {
            field: default_member_sort_field(),
            direction: default_ascending(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileMemberFilterParameters {
    #[serde(default)]
    search: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewGroupPageParameters {
    run_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewDecisionSetParameters {
    operation_id: String,
    run_id: i64,
    group_id: i64,
    file_id: i64,
    decision: String,
    expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewLiveValidationParameters {
    operation_id: String,
    run_id: i64,
    group_id: i64,
    expected_review_revision: i64,
    scope: String,
    file_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewLiveHintParameters {
    run_id: i64,
    root_path: String,
    event_count: i64,
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewLiveRootOverflowParameters {
    operation_id: String,
    run_id: i64,
    root_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewLiveRootListParameters {
    run_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewLiveRootReconcileParameters {
    operation_id: String,
    run_id: i64,
    root_path: String,
    expected_dirty_revision: i64,
    expected_review_revision: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFolderDecisionSetParameters {
    operation_id: String,
    run_id: i64,
    folder_group_id: i64,
    folder_member_id: i64,
    decision: String,
    expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreflightStartParameters {
    operation_id: String,
    run_id: i64,
    expected_review_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreflightGetParameters {
    #[serde(default)]
    preflight_id: Option<i64>,
    #[serde(default)]
    run_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreflightItemPageParameters {
    preflight_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreflightCancelParameters {
    preflight_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleOperationPrepareParameters {
    operation_id: String,
    run_id: i64,
    preflight_id: i64,
    expected_review_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleOperationGetParameters {
    #[serde(default)]
    recycle_operation_id: Option<i64>,
    #[serde(default)]
    run_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleOperationItemPageParameters {
    recycle_operation_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    result_status: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleEligibilityItemParameters {
    item_id: i64,
    status: String,
    #[serde(default)]
    reason_code: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleEligibilityReportParameters {
    report_operation_id: String,
    recycle_operation_id: i64,
    items: Vec<RecycleEligibilityItemParameters>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleOperationConfirmParameters {
    report_operation_id: String,
    recycle_operation_id: i64,
    confirmation_signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleOperationIdParameters {
    recycle_operation_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleBatchBeginParameters {
    report_operation_id: String,
    recycle_operation_id: i64,
    batch_id: i64,
    shell_attempt_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleResultItemParameters {
    item_id: i64,
    status: String,
    #[serde(default)]
    reason_code: Option<String>,
    #[serde(default)]
    shell_hresult: Option<i64>,
    #[serde(default)]
    recycled_item_present: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecycleBatchReportParameters {
    report_operation_id: String,
    recycle_operation_id: i64,
    batch_id: i64,
    items: Vec<RecycleResultItemParameters>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryReviewGetParameters {
    recycle_operation_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryReviewObservationPageParameters {
    recycle_operation_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    current_only: bool,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryReviewObservationRecordParameters {
    request_id: String,
    recycle_operation_id: i64,
    item_id: i64,
    observation: String,
    observed_at: String,
    #[serde(default)]
    note: Option<String>,
    evidence_version: i64,
    #[serde(default)]
    supersedes_observation_id: Option<i64>,
    #[serde(default)]
    correction_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceRuleIdParameters {
    rule_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceRuleSaveParameters {
    operation_id: String,
    #[serde(default)]
    rule_id: Option<i64>,
    name: String,
    roots: Vec<String>,
    expected_revision: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreferencePreviewScopeParameters {
    kind: String,
    #[serde(default)]
    group_ids: Vec<i64>,
    #[serde(default)]
    filter: DuplicateFileGroupFilterParameters,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferencePreviewParameters {
    run_id: i64,
    rule_id: i64,
    rule_revision: i64,
    review_revision: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    scope: PreferencePreviewScopeParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceApplyParameters {
    operation_id: String,
    run_id: i64,
    rule_id: i64,
    rule_revision: i64,
    source_review_revision: i64,
    preview_signature: String,
    scope: PreferencePreviewScopeParameters,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceApplicationPageParameters {
    run_id: i64,
    #[serde(default)]
    rule_id: Option<i64>,
    #[serde(default = "default_application_state")]
    state: String,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceApplicationGetParameters {
    run_id: i64,
    application_id: i64,
}

fn default_application_state() -> String {
    "all".to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferenceApplicationReverseParameters {
    operation_id: String,
    run_id: i64,
    application_id: i64,
    expected_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderGroupPageParameters {
    run_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    sort: DuplicateFolderGroupSortParameters,
    #[serde(default)]
    filter: DuplicateFolderGroupFilterParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderGroupSortParameters {
    #[serde(default = "default_folder_group_sort_field")]
    field: String,
    #[serde(default = "default_descending")]
    direction: String,
}

impl Default for DuplicateFolderGroupSortParameters {
    fn default() -> Self {
        Self {
            field: default_folder_group_sort_field(),
            direction: default_descending(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderGroupFilterParameters {
    #[serde(default)]
    search: Option<String>,
    #[serde(default = "default_minimum_size")]
    minimum_size: String,
}

impl Default for DuplicateFolderGroupFilterParameters {
    fn default() -> Self {
        Self {
            search: None,
            minimum_size: default_minimum_size(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderMemberPageParameters {
    run_id: i64,
    group_id: i64,
    #[serde(default = "default_result_page_size")]
    page_size: i64,
    #[serde(default)]
    sort: DuplicateFolderMemberSortParameters,
    #[serde(default)]
    filter: DuplicateFolderMemberFilterParameters,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderMemberSortParameters {
    #[serde(default = "default_member_sort_field")]
    field: String,
    #[serde(default = "default_ascending")]
    direction: String,
}

impl Default for DuplicateFolderMemberSortParameters {
    fn default() -> Self {
        Self {
            field: default_member_sort_field(),
            direction: default_ascending(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderMemberFilterParameters {
    #[serde(default)]
    search: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorPayload {
    version: u8,
    kind: String,
    query: String,
    before: bool,
    value: CursorScalar,
    id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum CursorScalar {
    Integer(String),
    Text(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDto {
    id: i64,
    name: String,
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
    cloud_policy: CloudPolicy,
    manual_location_exclusions: Vec<String>,
    registered_cloud_locations: Vec<RegisteredCloudLocation>,
    cloud_detection_status: CloudDetectionStatus,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunDto {
    id: i64,
    session_id: i64,
    parameters: RunParametersDto,
    status: String,
    phase: Option<String>,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    files_discovered: i64,
    bytes_discovered: String,
    files_hashed: i64,
    duplicate_file_groups: i64,
    duplicate_folder_groups: i64,
    wasted_bytes: String,
    warning_count: i64,
    excluded_subtree_count: i64,
    error_message: Option<String>,
    engine_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunParametersDto {
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
    directory_similarity_threshold_millis: u16,
    cloud_policy: CloudPolicy,
    manual_location_exclusions: Vec<String>,
    registered_cloud_locations: Vec<RegisteredCloudLocation>,
    cloud_detection_status: CloudDetectionStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunExclusionDto {
    id: i64,
    run_id: i64,
    path: String,
    reason_code: String,
    provider_id: Option<String>,
    provider_name: Option<String>,
    occurrence_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunWarningAggregateDto {
    id: i64,
    run_id: i64,
    phase: String,
    category: String,
    code: String,
    severity: String,
    message: String,
    occurrence_count: i64,
    examples: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileGroupDto {
    id: i64,
    run_id: i64,
    group_size: String,
    copy_count: i64,
    recoverable_bytes: String,
    representative_name: String,
    representative_type: String,
    distinct_selected_root_count: i64,
    distinct_drive_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileReviewSummaryDto {
    matching_group_count: i64,
    matching_copy_count: i64,
    potential_recoverable_bytes: String,
    largest_recoverable_bytes: String,
    distinct_selected_root_count: i64,
    distinct_drive_count: i64,
    across_drive_group_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileSelectedRootFacetDto {
    value: String,
    matching_group_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileDriveFacetDto {
    value: String,
    matching_group_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileMemberDto {
    id: i64,
    group_id: i64,
    path: String,
    file_name: String,
    parent_path: String,
    root_path: String,
    relative_path: String,
    drive_letter: String,
    size: String,
    modified_time_unix_nanos: String,
    decision: String,
    decision_provenance: Option<String>,
    decision_at: Option<String>,
    decision_application_id: Option<i64>,
    validation_state: Option<String>,
    validation_reason_code: Option<String>,
    validation_observed_at: Option<String>,
    invalidated_decision: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPlanDto {
    id: Option<i64>,
    run_id: i64,
    state: String,
    revision: i64,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPlanSummaryDto {
    decided_group_count: i64,
    keep_count: i64,
    remove_count: i64,
    undecided_count: i64,
    decided_folder_group_count: i64,
    folder_keep_count: i64,
    folder_remove_count: i64,
    folder_undecided_count: i64,
    effective_removal_file_count: i64,
    planned_removal_physical_item_count: i64,
    planned_removal_bytes: String,
    remaining_physical_copy_count: i64,
    intact_folder_copy_count: i64,
    rule_keep_count: i64,
    rule_remove_count: i64,
    active_rule_application_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewGroupSummaryDto {
    group_id: i64,
    keep_count: i64,
    remove_count: i64,
    undecided_count: i64,
    remaining_physical_copy_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFolderGroupSummaryDto {
    folder_group_id: i64,
    keep_count: i64,
    remove_count: i64,
    undecided_count: i64,
    intact_copy_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderGroupDto {
    id: i64,
    run_id: i64,
    total_bytes: String,
    descendant_file_count: i64,
    copy_count: i64,
    representative_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFolderMemberDto {
    id: i64,
    group_id: i64,
    path: String,
    decision: String,
    decision_provenance: Option<String>,
    decision_at: Option<String>,
}

struct ActiveRun {
    run_id: i64,
    cancel_token: Arc<AtomicBool>,
}

struct ActivePreflight {
    preflight_id: i64,
    cancel_token: Arc<AtomicBool>,
}

struct SharedState {
    database_path: PathBuf,
    status_database_path: PathBuf,
    diagnostic_log_path: Option<PathBuf>,
    active: Mutex<Option<ActiveRun>>,
    active_preflight: Mutex<Option<ActivePreflight>>,
    work_gate: Mutex<()>,
    idle: Condvar,
    output: Sender<String>,
}

impl SharedState {
    fn new(options: WorkerOptions, output: Sender<String>) -> Result<Arc<Self>, WorkerError> {
        let database_path = options.database_path;
        let status_database_path = options.status_database_path;
        let diagnostic_log_path = options.diagnostic_log_path;
        Database::open(&database_path.to_string_lossy()).map_err(|error| {
            WorkerError::Startup(format!("worker database initialization failed: {error}"))
        })?;
        Ok(Arc::new(Self {
            database_path,
            status_database_path,
            diagnostic_log_path,
            active: Mutex::new(None),
            active_preflight: Mutex::new(None),
            work_gate: Mutex::new(()),
            idle: Condvar::new(),
            output,
        }))
    }

    fn database(&self) -> Result<Database, ProtocolFailure> {
        Database::open_connection(&self.database_path.to_string_lossy())
            .map_err(internal_database_error)
    }

    fn status_database(&self) -> Result<StatusDatabase, ProtocolFailure> {
        StatusDatabase::open_reader(&self.status_database_path.to_string_lossy())
            .map_err(|error| ProtocolFailure::new("database_error", error.to_string()))
    }

    fn active_run_id(&self) -> Option<i64> {
        self.active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|active| active.run_id)
    }

    fn active_preflight_id(&self) -> Option<i64> {
        self.active_preflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|active| active.preflight_id)
    }

    fn emit<T: Serialize>(&self, event: &'static str, data: &T) {
        match serde_json::to_string(&json!({"type":"event", "event":event, "data":data})) {
            Ok(frame) => {
                let _ = self.output.send(frame);
            }
            Err(error) => eprintln!("worker event serialization failed: {error}"),
        }
    }

    fn shutdown(&self) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(run) = active.as_ref() {
            run.cancel_token.store(true, Ordering::Release);
            if let Ok(db) = Database::open_connection(&self.database_path.to_string_lossy()) {
                let _ = db.mark_run_cancelling(run.run_id);
            }
        }
        while active.is_some() {
            active = self
                .idle
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        drop(active);
        let mut preflight = self
            .active_preflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(active) = preflight.as_ref() {
            active.cancel_token.store(true, Ordering::Release);
            if let Ok(db) = Database::open_connection(&self.database_path.to_string_lossy()) {
                let _ = db.mark_preflight_cancelling(active.preflight_id);
            }
        }
        while preflight.is_some() {
            preflight = self
                .idle
                .wait(preflight)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    fn finish_active(&self, run_id: i64) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.as_ref().is_some_and(|run| run.run_id == run_id) {
            *active = None;
            self.idle.notify_all();
        }
    }

    fn finish_active_preflight(&self, preflight_id: i64) {
        let mut active = self
            .active_preflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active
            .as_ref()
            .is_some_and(|preflight| preflight.preflight_id == preflight_id)
        {
            *active = None;
            self.idle.notify_all();
        }
    }
}

pub struct WorkerSession {
    negotiated_protocol: Option<u32>,
    state: Arc<SharedState>,
}

impl WorkerSession {
    fn new(state: Arc<SharedState>) -> Self {
        Self {
            negotiated_protocol: None,
            state,
        }
    }

    pub fn handle_line(&mut self, line: &str) -> Result<String, WorkerError> {
        if line.is_empty() {
            return Err(WorkerError::FatalProtocol(
                "received an empty protocol frame".to_owned(),
            ));
        }
        if line.len() > MAXIMUM_FRAME_BYTES {
            return Err(WorkerError::FatalProtocol(format!(
                "input frame exceeds {MAXIMUM_FRAME_BYTES} bytes"
            )));
        }

        let value: Value = serde_json::from_str(line).map_err(|error| {
            WorkerError::FatalProtocol(format!("received malformed JSON: {error}"))
        })?;
        let object = value.as_object().ok_or_else(|| {
            WorkerError::FatalProtocol("protocol frame must be a JSON object".to_owned())
        })?;
        let id = request_id(object)?.to_owned();
        let request: RequestEnvelope = match serde_json::from_value(value) {
            Ok(request) => request,
            Err(error) => {
                return serialize_failure(
                    &id,
                    ProtocolFailure::new(
                        "invalid_request",
                        format!("Invalid request envelope: {error}"),
                    ),
                )
            }
        };

        if request.message_type != "request"
            || request.id.is_empty()
            || request.method.is_empty()
            || !request.params.is_object()
        {
            return serialize_failure(
                &request.id,
                ProtocolFailure::new(
                    "invalid_request",
                    "Expected a request with non-empty id/method and object params",
                ),
            );
        }

        if request.method == "hello" {
            return self.handle_hello(&request);
        }
        if self.negotiated_protocol.is_none() {
            return serialize_failure(
                &request.id,
                ProtocolFailure::new(
                    "handshake_required",
                    "hello must succeed before other requests",
                ),
            );
        }

        match self.dispatch(&request) {
            Ok(result) => serialize_success(&request.id, result),
            Err(error) => serialize_failure(&request.id, error),
        }
    }

    fn handle_hello(&mut self, request: &RequestEnvelope) -> Result<String, WorkerError> {
        if self.negotiated_protocol.is_some() {
            return serialize_failure(
                &request.id,
                ProtocolFailure::new("invalid_state", "hello has already completed"),
            );
        }
        let parameters: HelloParameters = match parse_parameters(request) {
            Ok(parameters) => parameters,
            Err(error) => return serialize_failure(&request.id, error),
        };
        if parameters.protocol_versions.is_empty()
            || parameters.protocol_versions.contains(&0)
            || parameters.client.name.trim().is_empty()
            || parameters.client.version.trim().is_empty()
        {
            return serialize_failure(
                &request.id,
                ProtocolFailure::new(
                    "invalid_request",
                    "hello requires positive protocolVersions and non-empty client name/version",
                ),
            );
        }
        if !parameters.protocol_versions.contains(&PROTOCOL_VERSION) {
            return serialize_failure(
                &request.id,
                ProtocolFailure::new(
                    "unsupported_protocol",
                    "No mutually supported protocol version",
                )
                .with_details(json!({"workerProtocolVersions":[PROTOCOL_VERSION]})),
            );
        }
        self.negotiated_protocol = Some(PROTOCOL_VERSION);
        serialize_success(
            &request.id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "workerVersion": env!("CARGO_PKG_VERSION"),
                "engineVersion": super_duper_core::ENGINE_VERSION,
            }),
        )
    }

    fn dispatch(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        match request.method.as_str() {
            "app.status" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "activeRunId": self.state.active_run_id(),
                "activePreflightId": self.state.active_preflight_id(),
            })),
            "session.list" => self.session_list(request),
            "session.get" => self.session_get(request),
            "session.create" => self.session_create(request),
            "session.update" => self.session_update(request),
            "session.delete" => self.session_delete(request),
            "run.list" => self.run_list(request),
            "run.get" => self.run_get(request),
            "run_exclusion.page" => self.run_exclusion_page(request),
            "warning.page" => self.warning_page(request),
            "performance.run.page" => self.performance_run_page(request),
            "performance.snapshot.get" => self.performance_snapshot_get(request),
            "run.start" => self.run_start(request),
            "run.cancel" => self.run_cancel(request),
            "review_plan.get" => self.review_plan_get(request),
            "review_group.page" => self.review_group_page(request),
            "review_folder_group.page" => self.review_folder_group_page(request),
            "review_decision.set" => self.review_decision_set(request),
            "review_folder_decision.set" => self.review_folder_decision_set(request),
            "preflight.start" => self.preflight_start(request),
            "preflight.get" => self.preflight_get(request),
            "preflight.item.page" => self.preflight_item_page(request),
            "preflight.cancel" => self.preflight_cancel(request),
            "recycle_operation.prepare" => self.recycle_operation_prepare(request),
            "recycle_operation.get" => self.recycle_operation_get(request),
            "recycle_operation.item.page" => self.recycle_operation_item_page(request),
            "recycle_operation.eligibility.report" => {
                self.recycle_operation_eligibility_report(request)
            }
            "recycle_operation.confirm" => self.recycle_operation_confirm(request),
            "recycle_operation.cancel" => self.recycle_operation_cancel(request),
            "recycle_operation.batch.next" => self.recycle_operation_batch_next(request),
            "recycle_operation.batch.begin" => self.recycle_operation_batch_begin(request),
            "recycle_operation.batch.report" => self.recycle_operation_batch_report(request),
            "recovery_review.get" => self.recovery_review_get(request),
            "recovery_review.observation.page" => self.recovery_review_observation_page(request),
            "recovery_review.observation.record" => {
                self.recovery_review_observation_record(request)
            }
            "preference_rule.list" => self.preference_rule_list(request),
            "preference_rule.get" => self.preference_rule_get(request),
            "preference_rule.save" => self.preference_rule_save(request),
            "preference_rule.preview" => self.preference_rule_preview(request),
            "preference_rule.apply" => self.preference_rule_apply(request),
            "preference_rule.application.get" => self.preference_rule_application_get(request),
            "preference_rule.application.page" => self.preference_rule_application_page(request),
            "preference_rule.application.reverse" => {
                self.preference_rule_application_reverse(request)
            }
            "duplicate_file_group.page" => self.duplicate_file_group_page(request),
            "duplicate_file_selected_root_facet.page" => {
                self.duplicate_file_selected_root_facet_page(request)
            }
            "duplicate_file_drive_facet.page" => self.duplicate_file_drive_facet_page(request),
            "duplicate_file_group.members" => self.duplicate_file_group_members(request),
            "review_live_validation.run" => self.review_live_validation_run(request),
            "review_live_hint.batch" => self.review_live_hint_batch(request),
            "review_live_root.overflow" => self.review_live_root_overflow(request),
            "review_live_root.list" => self.review_live_root_list(request),
            "review_live_root.reconcile" => self.review_live_root_reconcile(request),
            "duplicate_folder_group.page" => self.duplicate_folder_group_page(request),
            "duplicate_folder_group.members" => self.duplicate_folder_group_members(request),
            _ => Err(ProtocolFailure::new(
                "method_not_found",
                format!("Unknown method: {}", request.method),
            )),
        }
    }

    fn session_list(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let page: PageParameters = parse_parameters(request)?;
        validate_page(&page)?;
        let db = self.state.database()?;
        let (sessions, total) = db
            .list_sessions(page.offset, page.limit)
            .map_err(internal_database_error)?;
        let sessions = sessions
            .into_iter()
            .map(session_dto)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({"sessions":sessions, "total":total}))
    }

    fn session_get(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: IdParameters = parse_parameters(request)?;
        let session = get_session(&self.state.database()?, parameters.session_id)?;
        Ok(json!({"session":session_dto(session)?}))
    }

    fn session_create(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        self.ensure_session_mutation_allowed()?;
        let parameters: SessionWriteParameters = parse_parameters(request)?;
        let validated = validate_session(
            parameters.name,
            parameters.roots,
            parameters.ignore_patterns,
            parameters.cloud_policy,
            parameters.manual_location_exclusions,
            parameters.registered_cloud_locations,
            parameters.cloud_detection_status,
        )?;
        let db = self.state.database()?;
        if session_name_exists(&db, &validated.name, None)? {
            return Err(ProtocolFailure::new(
                "session_name_conflict",
                "A session with that name already exists",
            ));
        }
        let id = db
            .create_session_with_cloud_settings(
                &validated.name,
                &validated.roots,
                &validated.ignore_patterns,
                validated.cloud_policy,
                &validated.manual_location_exclusions,
                &validated.registered_cloud_locations,
                validated.cloud_detection_status,
            )
            .map_err(internal_database_error)?;
        Ok(json!({"session":session_dto(get_session(&db, id)?)?}))
    }

    fn session_update(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        self.ensure_session_mutation_allowed()?;
        let parameters: SessionUpdateParameters = parse_parameters(request)?;
        let validated = validate_session(
            parameters.name,
            parameters.roots,
            parameters.ignore_patterns,
            parameters.cloud_policy,
            parameters.manual_location_exclusions,
            parameters.registered_cloud_locations,
            parameters.cloud_detection_status,
        )?;
        let db = self.state.database()?;
        let _ = get_session(&db, parameters.session_id)?;
        if session_name_exists(&db, &validated.name, Some(parameters.session_id))? {
            return Err(ProtocolFailure::new(
                "session_name_conflict",
                "A session with that name already exists",
            ));
        }
        db.update_session_with_cloud_settings(
            parameters.session_id,
            &validated.name,
            &validated.roots,
            &validated.ignore_patterns,
            validated.cloud_policy,
            &validated.manual_location_exclusions,
            &validated.registered_cloud_locations,
            validated.cloud_detection_status,
        )
        .map_err(internal_database_error)?;
        Ok(json!({"session":session_dto(get_session(&db, parameters.session_id)?)?}))
    }

    fn session_delete(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        self.ensure_session_mutation_allowed()?;
        let parameters: IdParameters = parse_parameters(request)?;
        let db = self.state.database()?;
        let _ = get_session(&db, parameters.session_id)?;
        db.delete_session(parameters.session_id)
            .map_err(internal_database_error)?;
        Ok(json!({"sessionId":parameters.session_id}))
    }

    fn ensure_session_mutation_allowed(&self) -> Result<(), ProtocolFailure> {
        if let Some(run_id) = self.state.active_run_id() {
            Err(ProtocolFailure::new(
                "invalid_state",
                "Session definitions cannot change while a scan is active",
            )
            .with_details(json!({"activeRunId":run_id})))
        } else {
            Ok(())
        }
    }

    fn run_list(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: RunListParameters = parse_parameters(request)?;
        validate_page_values(parameters.offset, parameters.limit)?;
        let db = self.state.database()?;
        let (runs, total) = if let Some(session_id) = parameters.session_id {
            let _ = get_session(&db, session_id)?;
            db.list_session_runs(session_id, parameters.offset, parameters.limit)
        } else {
            db.list_runs(parameters.offset, parameters.limit)
        }
        .map_err(internal_database_error)?;
        let runs = runs
            .into_iter()
            .map(run_dto)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({"runs":runs, "total":total}))
    }

    fn run_get(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: RunIdParameters = parse_parameters(request)?;
        let run = get_run(&self.state.database()?, parameters.run_id)?;
        Ok(json!({"run":run_dto(run)?}))
    }

    fn run_exclusion_page(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: RunExclusionPageParameters = parse_parameters(request)?;
        validate_page_values(parameters.offset, parameters.limit)?;
        let db = self.state.database()?;
        let _ = get_run(&db, parameters.run_id)?;
        let (exclusions, total) = db
            .page_run_exclusions(parameters.run_id, parameters.offset, parameters.limit)
            .map_err(internal_database_error)?;
        Ok(json!({
            "exclusions": exclusions.into_iter().map(run_exclusion_dto).collect::<Vec<_>>(),
            "total": total,
        }))
    }

    fn warning_page(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: WarningPageParameters = parse_parameters(request)?;
        if parameters.run_id <= 0 || !(1..=MAXIMUM_PAGE_SIZE).contains(&parameters.page_size) {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "warning.page requires a positive runId and pageSize between 1 and 500",
            ));
        }
        let sort_field = parse_warning_sort_field(&parameters.sort.field)?;
        let sort_direction = parse_sort_direction(&parameters.sort.direction)?;
        let db = self.state.database()?;
        let _ = get_run(&db, parameters.run_id)?;
        let cursor_identity = if parameters.cursor.is_some() {
            Some(
                db.run_warning_snapshot_identity(parameters.run_id)
                    .map_err(internal_database_error)?,
            )
        } else {
            None
        };
        let cursor = match &cursor_identity {
            Some((revision, run_status)) => {
                let signature = warning_query_signature(
                    parameters.run_id,
                    sort_field,
                    sort_direction,
                    *revision,
                    run_status,
                );
                decode_cursor(parameters.cursor.as_deref(), "run-warnings", &signature)?
            }
            None => None,
        };
        validate_cursor_value(
            cursor.as_ref(),
            sort_field != RunWarningSortField::OccurrenceCount,
        )?;
        if cursor.as_ref().is_some_and(|cursor| cursor.before) {
            return Err(invalid_cursor());
        }
        let query_started = Instant::now();
        let mut snapshot = db
            .page_run_warning_snapshot(&RunWarningPageQuery {
                run_id: parameters.run_id,
                limit: parameters.page_size + 1,
                sort_field,
                sort_direction,
                cursor,
            })
            .map_err(internal_database_error)?;
        if cursor_identity.as_ref().is_some_and(|(revision, status)| {
            *revision != snapshot.revision || status != &snapshot.run_status
        }) {
            return Err(invalid_cursor());
        }
        let signature = warning_query_signature(
            parameters.run_id,
            sort_field,
            sort_direction,
            snapshot.revision,
            &snapshot.run_status,
        );
        let has_more = snapshot.warnings.len() > parameters.page_size as usize;
        if has_more {
            snapshot.warnings.truncate(parameters.page_size as usize);
        }
        log_result_query(
            "warning.page",
            parameters.run_id,
            None,
            parameters.page_size,
            snapshot.warnings.len(),
            snapshot.total,
            query_started.elapsed(),
        );
        let next_cursor = if has_more {
            snapshot
                .warnings
                .last()
                .map(|warning| encode_warning_cursor(warning, sort_field, &signature))
                .transpose()?
        } else {
            None
        };
        Ok(json!({
            "warnings": snapshot.warnings.into_iter().map(run_warning_aggregate_dto).collect::<Vec<_>>(),
            "total": snapshot.total,
            "warningCount": snapshot.warning_count,
            "accountedWarningCount": snapshot.accounted_warning_count,
            "snapshotRevision": snapshot.revision,
            "snapshotState": warning_snapshot_state(&snapshot.run_status),
            "runStatus": snapshot.run_status,
            "diagnosticLog": diagnostic_log_metadata(self.state.diagnostic_log_path.as_deref()),
            "nextCursor": next_cursor,
            "executorEnabled": false,
        }))
    }

    fn performance_run_page(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: PerformanceRunPageParameters = parse_parameters(request)?;
        if !(1..=PERFORMANCE_HISTORY_PAGE_SIZE).contains(&parameters.page_size)
            || parameters.before_id.is_some_and(|id| id <= 0)
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "performance.run.page requires pageSize between 1 and 25 and a positive beforeId",
            ));
        }
        let database = self.state.status_database()?;
        let runs = database
            .list_runs(parameters.before_id, parameters.page_size as usize)
            .map_err(|error| ProtocolFailure::new("database_error", error.to_string()))?;
        let next_before_id = (runs.len() == parameters.page_size as usize)
            .then(|| runs.last().map(|run| run.id))
            .flatten();
        Ok(json!({
            "runs": runs,
            "nextBeforeId": next_before_id,
            "executorEnabled": false,
        }))
    }

    fn performance_snapshot_get(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: PerformanceSnapshotParameters = parse_parameters(request)?;
        if parameters.status_run_id.is_some() == parameters.product_run_id.is_some()
            || parameters.status_run_id.is_some_and(|id| id <= 0)
            || parameters.product_run_id.is_some_and(|id| id <= 0)
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "performance.snapshot.get requires exactly one positive statusRunId or productRunId",
            ));
        }
        let database = self.state.status_database()?;
        let run = match parameters.status_run_id {
            Some(id) => database.get_run(id),
            None => database.get_run_by_product_run_id(parameters.product_run_id.unwrap()),
        }
        .map_err(|error| ProtocolFailure::new("database_error", error.to_string()))?;
        let counters = database
            .get_run_counters(run.id)
            .map_err(|error| ProtocolFailure::new("database_error", error.to_string()))?;
        let phases = database
            .get_run_phases(run.id)
            .map_err(|error| ProtocolFailure::new("database_error", error.to_string()))?;
        let host = database
            .get_host_performance_summary(run.id)
            .map_err(|error| ProtocolFailure::new("database_error", error.to_string()))?;
        let devices = database
            .get_device_performance_summaries(run.id)
            .map_err(|error| ProtocolFailure::new("database_error", error.to_string()))?;
        Ok(json!({
            "run": run,
            "counters": counters,
            "phases": phases,
            "host": host,
            "devices": devices,
            "executorEnabled": false,
        }))
    }

    fn review_plan_get(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: RunIdParameters = parse_parameters(request)?;
        let db = self.state.database()?;
        let query_started = Instant::now();
        let view = db
            .get_review_plan_view(parameters.run_id)
            .map_err(review_error)?;
        log_result_query(
            "review_plan.get",
            parameters.run_id,
            None,
            1,
            1,
            1,
            query_started.elapsed(),
        );
        Ok(json!({
            "plan": review_plan_dto(parameters.run_id, &view),
            "summary": review_plan_summary_dto(&view.summary),
        }))
    }

    fn preference_rule_list(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let page: PageParameters = parse_parameters(request)?;
        validate_page(&page)?;
        if page.limit > 200 {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "preference_rule.list limit must be 1..=200",
            )
            .with_details(json!({"field":"limit","maximum":200})));
        }
        let (rules, total) = self
            .state
            .database()?
            .list_preference_rules(page.offset, page.limit)
            .map_err(preference_error)?;
        Ok(json!({
            "rules": rules.iter().map(preference_rule_summary_dto).collect::<Vec<_>>(),
            "total": total,
        }))
    }

    fn preference_rule_get(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: PreferenceRuleIdParameters = parse_parameters(request)?;
        let rule = self
            .state
            .database()?
            .get_preference_rule(parameters.rule_id)
            .map_err(preference_error)?;
        Ok(json!({"rule":preference_rule_dto(&rule)}))
    }

    fn preference_rule_save(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: PreferenceRuleSaveParameters = parse_parameters(request)?;
        validate_preference_rule_parameters(&parameters)?;
        let result = self
            .state
            .database()?
            .save_preference_rule(
                &parameters.operation_id,
                parameters.rule_id,
                &parameters.name,
                &parameters.roots,
                parameters.expected_revision,
            )
            .map_err(preference_error)?;
        Ok(json!({
            "rule": preference_rule_dto(&result.rule),
            "replayed": result.replayed,
        }))
    }

    fn preference_rule_preview(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let mut parameters: PreferencePreviewParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        if parameters.rule_revision <= 0 || parameters.review_revision < 0 {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "ruleRevision must be positive and reviewRevision must be non-negative",
            ));
        }
        let (scope, scope_signature) = parse_preference_preview_scope(&mut parameters.scope)?;
        let plan = self
            .state
            .database()?
            .get_review_plan_view(parameters.run_id)
            .map_err(review_error)?;
        let plan_id = plan.plan.as_ref().map_or(0, |value| value.id);
        let current_review_revision = plan.plan.as_ref().map_or(0, |value| value.revision);
        let signature = json!({
            "runId": parameters.run_id,
            "ruleId": parameters.rule_id,
            "ruleRevision": parameters.rule_revision,
            "reviewPlanId": plan_id,
            "reviewRevision": current_review_revision,
            "pageSize": parameters.page_size,
            "scope": scope_signature,
        })
        .to_string();
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "preference-rule-preview",
            &signature,
        )?;
        validate_cursor_value(cursor.as_ref(), false)?;
        if cursor.as_ref().is_some_and(|value| value.before) {
            return Err(ProtocolFailure::new(
                "invalid_cursor",
                "Preference-preview cursors support forward keyset paging only",
            ));
        }
        let started = Instant::now();
        let page = self
            .state
            .database()?
            .page_preference_preview(
                parameters.run_id,
                parameters.rule_id,
                parameters.rule_revision,
                parameters.review_revision,
                &scope,
                parameters.page_size,
                cursor.as_ref().map(|value| value.id),
            )
            .map_err(preference_error)?;
        log_result_query(
            "preference_rule.preview",
            parameters.run_id,
            None,
            parameters.page_size,
            page.groups.len(),
            page.total,
            started.elapsed(),
        );
        let next_cursor = page
            .groups
            .last()
            .filter(|_| page.has_more)
            .map(|group| encode_preference_preview_cursor(group.group_id, &signature))
            .transpose()?;
        Ok(json!({
            "groups": page.groups.iter().map(preference_preview_group_dto).collect::<Vec<_>>(),
            "total": page.total,
            "nextCursor": next_cursor,
            "ruleId": page.rule_id,
            "ruleRevision": page.rule_revision,
            "reviewPlanId": page.review_plan_id,
            "reviewRevision": page.review_revision,
            "previewSignature": page.preview_signature,
            "summary": preference_preview_summary_dto(&page.summary),
        }))
    }

    fn preference_rule_apply(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let mut parameters: PreferenceApplyParameters = parse_parameters(request)?;
        validate_operation_id(&parameters.operation_id)?;
        if parameters.rule_revision <= 0 || parameters.source_review_revision < 0 {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "ruleRevision must be positive and sourceReviewRevision must be non-negative",
            ));
        }
        if parameters.preview_signature.is_empty()
            || parameters.preview_signature.chars().count() > 128
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "previewSignature must contain 1 to 128 characters",
            ));
        }
        let (scope, _) = parse_preference_preview_scope(&mut parameters.scope)?;
        let result = self
            .state
            .database()?
            .apply_preference_rule(
                &parameters.operation_id,
                parameters.run_id,
                parameters.rule_id,
                parameters.rule_revision,
                parameters.source_review_revision,
                &parameters.preview_signature,
                &scope,
            )
            .map_err(preference_error)?;
        Ok(json!({
            "application": preference_application_dto(&result.application),
            "replayed": result.replayed,
        }))
    }

    fn preference_rule_application_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: PreferenceApplicationPageParameters = parse_parameters(request)?;
        if !(1..=200).contains(&parameters.page_size) {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "application pageSize must be 1..=200",
            ));
        }
        if parameters.rule_id.is_some_and(|id| id <= 0)
            || !matches!(parameters.state.as_str(), "all" | "active" | "reversed")
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "ruleId must be positive and state must be active, reversed, or all",
            ));
        }
        let plan = self
            .state
            .database()?
            .get_review_plan_view(parameters.run_id)
            .map_err(review_error)?;
        let signature = json!({
            "runId":parameters.run_id,
            "ruleId":parameters.rule_id,
            "state":parameters.state,
            "planId":plan.plan.as_ref().map(|value| value.id),
            "revision":plan.plan.as_ref().map_or(0, |value| value.revision),
            "pageSize":parameters.page_size,
        })
        .to_string();
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "preference-rule-applications",
            &signature,
        )?;
        validate_cursor_value(cursor.as_ref(), false)?;
        if cursor.as_ref().is_some_and(|value| value.before) {
            return Err(invalid_cursor());
        }
        let page = self
            .state
            .database()?
            .page_preference_applications(
                parameters.run_id,
                parameters.rule_id,
                Some(&parameters.state),
                parameters.page_size,
                cursor.as_ref().map(|value| value.id),
            )
            .map_err(preference_error)?;
        let next_cursor = page
            .applications
            .last()
            .filter(|_| page.has_more)
            .map(|application| encode_preference_application_cursor(application.id, &signature))
            .transpose()?;
        Ok(json!({
            "applications":page.applications.iter().map(preference_application_summary_dto).collect::<Vec<_>>(),
            "total":page.total,
            "nextCursor":next_cursor,
            "planId":page.plan_id,
            "revision":page.revision,
        }))
    }

    fn preference_rule_application_get(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: PreferenceApplicationGetParameters = parse_parameters(request)?;
        if parameters.run_id <= 0 || parameters.application_id <= 0 {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "runId and applicationId must be positive",
            ));
        }
        let application = self
            .state
            .database()?
            .get_preference_application(parameters.run_id, parameters.application_id)
            .map_err(preference_error)?;
        Ok(json!({"application":preference_application_dto(&application)}))
    }

    fn preference_rule_application_reverse(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: PreferenceApplicationReverseParameters = parse_parameters(request)?;
        validate_operation_id(&parameters.operation_id)?;
        if parameters.run_id <= 0
            || parameters.application_id <= 0
            || parameters.expected_revision < 0
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "runId/applicationId must be positive and expectedRevision non-negative",
            ));
        }
        let result = self
            .state
            .database()?
            .reverse_preference_rule_application(
                &parameters.operation_id,
                parameters.run_id,
                parameters.application_id,
                parameters.expected_revision,
            )
            .map_err(preference_error)?;
        Ok(json!({
            "applicationId":result.application_id,
            "planId":result.plan_id,
            "appliedRevision":result.applied_revision,
            "replayed":result.replayed,
            "state":"reversed",
            "removedRuleKeepCount":result.removed_keep_count,
            "removedRuleRemoveCount":result.removed_remove_count,
        }))
    }

    fn review_group_page(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewGroupPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        let plan = db
            .get_review_plan_view(parameters.run_id)
            .map_err(review_error)?;
        let plan_id = plan.plan.as_ref().map_or(0, |value| value.id);
        let revision = plan.plan.as_ref().map_or(0, |value| value.revision);
        let signature = format!(
            "{}|{}|{}|{}",
            parameters.run_id, plan_id, revision, parameters.page_size
        );
        let cursor = decode_cursor(parameters.cursor.as_deref(), "review-groups", &signature)?;
        validate_cursor_value(cursor.as_ref(), false)?;
        if cursor.as_ref().is_some_and(|value| value.before) {
            return Err(ProtocolFailure::new(
                "invalid_cursor",
                "Review-group cursors support forward keyset paging only",
            ));
        }
        let after_group_id = cursor.as_ref().map(|value| value.id);
        let query_started = Instant::now();
        let page = db
            .page_review_groups(parameters.run_id, parameters.page_size, after_group_id)
            .map_err(review_error)?;
        log_result_query(
            "review_group.page",
            parameters.run_id,
            None,
            parameters.page_size,
            page.groups.len(),
            page.total,
            query_started.elapsed(),
        );
        let next_cursor = page
            .groups
            .last()
            .filter(|_| page.has_more)
            .map(|group| encode_review_group_cursor(group.group_id, &signature))
            .transpose()?;
        Ok(json!({
            "groups": page.groups.iter().map(review_group_summary_dto).collect::<Vec<_>>(),
            "total": page.total,
            "planId": page.plan_id,
            "revision": page.revision,
            "nextCursor": next_cursor,
        }))
    }

    fn review_folder_group_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewGroupPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        let plan = db
            .get_review_plan_view(parameters.run_id)
            .map_err(review_error)?;
        let plan_id = plan.plan.as_ref().map_or(0, |value| value.id);
        let revision = plan.plan.as_ref().map_or(0, |value| value.revision);
        let signature = format!(
            "{}|{}|{}|{}|visible",
            parameters.run_id, plan_id, revision, parameters.page_size
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "review-folder-groups",
            &signature,
        )?;
        validate_cursor_value(cursor.as_ref(), false)?;
        if cursor.as_ref().is_some_and(|value| value.before) {
            return Err(ProtocolFailure::new(
                "invalid_cursor",
                "Review-folder-group cursors support forward keyset paging only",
            ));
        }
        let query_started = Instant::now();
        let page = db
            .page_review_folder_groups(
                parameters.run_id,
                parameters.page_size,
                cursor.as_ref().map(|value| value.id),
            )
            .map_err(review_error)?;
        log_result_query(
            "review_folder_group.page",
            parameters.run_id,
            None,
            parameters.page_size,
            page.groups.len(),
            page.total,
            query_started.elapsed(),
        );
        let next_cursor = page
            .groups
            .last()
            .filter(|_| page.has_more)
            .map(|group| encode_review_folder_group_cursor(group.folder_group_id, &signature))
            .transpose()?;
        Ok(json!({
            "groups": page.groups.iter().map(review_folder_group_summary_dto).collect::<Vec<_>>(),
            "total": page.total,
            "planId": page.plan_id,
            "revision": page.revision,
            "nextCursor": next_cursor,
        }))
    }

    fn review_decision_set(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewDecisionSetParameters = parse_parameters(request)?;
        let operation_id = parameters.operation_id.trim();
        if operation_id.is_empty() || operation_id.chars().count() > MAXIMUM_OPERATION_ID_CHARACTERS
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                format!(
                    "operationId must contain 1 to {MAXIMUM_OPERATION_ID_CHARACTERS} characters"
                ),
            )
            .with_details(json!({"field":"operationId"})));
        }
        if parameters.expected_revision < 0 {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "expectedRevision must be non-negative",
            )
            .with_details(json!({"field":"expectedRevision"})));
        }
        let decision = ReviewDecisionKind::parse(&parameters.decision).ok_or_else(|| {
            ProtocolFailure::new(
                "invalid_request",
                "decision must be keep, remove, or undecided",
            )
            .with_details(json!({
                "field":"decision",
                "allowed":["keep","remove","undecided"]
            }))
        })?;
        let result = self
            .state
            .database()?
            .set_review_decision(
                operation_id,
                parameters.run_id,
                parameters.group_id,
                parameters.file_id,
                decision,
                parameters.expected_revision,
            )
            .map_err(review_error)?;
        Ok(json!({
            "planId": result.plan_id,
            "appliedRevision": result.applied_revision,
            "replayed": result.replayed,
            "decision": result.decision.as_str(),
        }))
    }

    fn review_folder_decision_set(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewFolderDecisionSetParameters = parse_parameters(request)?;
        let operation_id = parameters.operation_id.trim();
        if operation_id.is_empty() || operation_id.chars().count() > MAXIMUM_OPERATION_ID_CHARACTERS
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                format!(
                    "operationId must contain 1 to {MAXIMUM_OPERATION_ID_CHARACTERS} characters"
                ),
            )
            .with_details(json!({"field":"operationId"})));
        }
        if parameters.expected_revision < 0 {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "expectedRevision must be non-negative",
            )
            .with_details(json!({"field":"expectedRevision"})));
        }
        let decision = ReviewDecisionKind::parse(&parameters.decision).ok_or_else(|| {
            ProtocolFailure::new(
                "invalid_request",
                "decision must be keep, remove, or undecided",
            )
            .with_details(json!({
                "field":"decision",
                "allowed":["keep","remove","undecided"]
            }))
        })?;
        let result = self
            .state
            .database()?
            .set_review_folder_decision(
                operation_id,
                parameters.run_id,
                parameters.folder_group_id,
                parameters.folder_member_id,
                decision,
                parameters.expected_revision,
            )
            .map_err(review_error)?;
        Ok(json!({
            "planId": result.plan_id,
            "appliedRevision": result.applied_revision,
            "replayed": result.replayed,
            "decision": result.decision.as_str(),
        }))
    }

    fn review_live_validation_run(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewLiveValidationParameters = parse_parameters(request)?;
        let result = self
            .state
            .database()?
            .validate_review_files(&ReviewLiveValidationRequest {
                operation_id: parameters.operation_id,
                run_id: parameters.run_id,
                group_id: parameters.group_id,
                expected_review_revision: parameters.expected_review_revision,
                scope: parameters.scope,
                file_ids: parameters.file_ids,
            })
            .map_err(review_live_validation_error)?;
        let present_count = result
            .items
            .iter()
            .filter(|item| item.state == "present")
            .count();
        let changed_count = result
            .items
            .iter()
            .filter(|item| item.state == "changed")
            .count();
        let missing_count = result
            .items
            .iter()
            .filter(|item| item.state == "missing")
            .count();
        let unavailable_count = result
            .items
            .iter()
            .filter(|item| item.state == "unavailable")
            .count();
        let invalidated_decision_count = result
            .items
            .iter()
            .filter(|item| item.decision_invalidated)
            .count();
        Ok(json!({
            "validationId": result.validation_id,
            "runId": result.run_id,
            "groupId": result.group_id,
            "reviewRevision": result.review_revision,
            "scope": result.scope,
            "replayed": result.replayed,
            "summary": {
                "itemCount": result.items.len(),
                "presentCount": present_count,
                "changedCount": changed_count,
                "missingCount": missing_count,
                "unavailableCount": unavailable_count,
                "invalidatedDecisionCount": invalidated_decision_count,
            },
            "items": result.items.into_iter().map(|item| json!({
                "fileId": item.file_id,
                "state": item.state,
                "reasonCode": item.reason_code,
                "decisionInvalidated": item.decision_invalidated,
                "invalidatedDecision": item.invalidated_decision.map(ReviewDecisionKind::as_str),
                "observedAt": item.observed_at,
            })).collect::<Vec<_>>(),
        }))
    }

    fn review_live_hint_batch(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewLiveHintParameters = parse_parameters(request)?;
        let result = self
            .state
            .database()?
            .resolve_review_live_hints(&ReviewLiveHintRequest {
                run_id: parameters.run_id,
                root_path: parameters.root_path,
                event_count: parameters.event_count,
                paths: parameters.paths,
            })
            .map_err(review_live_hint_error)?;
        let data = json!({
            "kind": "hints",
            "runId": result.run_id,
            "rootPath": result.root_path,
            "eventCount": result.event_count,
            "coalescedPathCount": result.coalesced_path_count,
            "items": result.items.into_iter().map(|item| json!({
                "fileId": item.file_id,
                "groupId": item.group_id,
                "path": item.path,
            })).collect::<Vec<_>>(),
            "executorEnabled": false,
        });
        self.state.emit("result.state_changed", &data);
        Ok(data)
    }

    fn review_live_root_overflow(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewLiveRootOverflowParameters = parse_parameters(request)?;
        let result = self
            .state
            .database()?
            .mark_review_root_overflow(&ReviewLiveRootOverflowRequest {
                operation_id: parameters.operation_id,
                run_id: parameters.run_id,
                root_path: parameters.root_path,
            })
            .map_err(review_live_root_error)?;
        let data = json!({
            "kind": "overflow",
            "runId": result.root.run_id,
            "rootPath": result.root.root_path,
            "eventCount": 0,
            "coalescedPathCount": 0,
            "items": [],
            "root": review_live_root_dto(&result.root),
            "replayed": result.replayed,
            "executorEnabled": false,
        });
        self.state.emit("result.state_changed", &data);
        Ok(data)
    }

    fn review_live_root_list(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewLiveRootListParameters = parse_parameters(request)?;
        let started = Instant::now();
        let roots = self
            .state
            .database()?
            .list_dirty_review_roots(parameters.run_id)
            .map_err(review_live_root_error)?;
        log_result_query(
            "review_live_root.list",
            parameters.run_id,
            None,
            64,
            roots.len(),
            roots.len() as i64,
            started.elapsed(),
        );
        Ok(json!({
            "runId": parameters.run_id,
            "roots": roots.iter().map(review_live_root_dto).collect::<Vec<_>>(),
            "total": roots.len(),
            "executorEnabled": false,
        }))
    }

    fn review_live_root_reconcile(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: ReviewLiveRootReconcileParameters = parse_parameters(request)?;
        let result = self
            .state
            .database()?
            .reconcile_review_root(&ReviewLiveRootReconciliationRequest {
                operation_id: parameters.operation_id,
                run_id: parameters.run_id,
                root_path: parameters.root_path,
                expected_dirty_revision: parameters.expected_dirty_revision,
                expected_review_revision: parameters.expected_review_revision,
                page_size: parameters.page_size,
            })
            .map_err(review_live_root_error)?;
        Ok(json!({
            "reconciliationId": result.reconciliation_id,
            "runId": result.run_id,
            "rootPath": result.root_path,
            "dirtyRevision": result.dirty_revision,
            "reviewRevision": result.review_revision,
            "replayed": result.replayed,
            "summary": {
                "itemCount": result.summary.item_count,
                "presentCount": result.summary.present_count,
                "changedCount": result.summary.changed_count,
                "missingCount": result.summary.missing_count,
                "unavailableCount": result.summary.unavailable_count,
                "invalidatedDecisionCount": result.summary.invalidated_decision_count,
            },
            "items": result.items.into_iter().map(|item| json!({
                "fileId": item.file_id,
                "state": item.state,
                "reasonCode": item.reason_code,
                "decisionInvalidated": item.decision_invalidated,
                "invalidatedDecision": item.invalidated_decision.map(ReviewDecisionKind::as_str),
                "observedAt": item.observed_at,
            })).collect::<Vec<_>>(),
            "root": review_live_root_dto(&result.root),
            "executorEnabled": false,
        }))
    }

    fn duplicate_file_group_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: DuplicateFileGroupPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        ensure_completed_result_run(&db, parameters.run_id)?;
        let sort_field = parse_group_sort_field(&parameters.sort.field)?;
        let sort_direction = parse_sort_direction(&parameters.sort.direction)?;
        let requested_path_match = parse_path_match(&parameters.filter.path_match)?;
        let search = validate_group_path_filter(parameters.filter.search, requested_path_match)?;
        let path_match = search
            .as_ref()
            .map_or(DuplicateFilePathMatchMode::Substring, |_| {
                requested_path_match
            });
        let extension = normalize_extension_filter(parameters.filter.extension.as_deref())?;
        let extension_match =
            normalize_extension_match(&parameters.filter.extension_match, extension.as_deref())?;
        let minimum_size =
            parse_non_negative_decimal(&parameters.filter.minimum_size, "filter.minimumSize")?;
        validate_minimum_copy_count(parameters.filter.minimum_copy_count)?;
        let selected_root = parameters
            .filter
            .selected_root
            .filter(|value| !value.is_empty());
        let selected_drive = parameters
            .filter
            .selected_drive
            .filter(|value| !value.is_empty());
        let signature = group_query_signature(
            parameters.run_id,
            sort_field,
            sort_direction,
            search.as_deref(),
            path_match,
            extension.as_deref(),
            extension_match,
            minimum_size,
            parameters.filter.minimum_copy_count,
            parameters.filter.across_drives,
            selected_root.as_deref(),
            selected_drive.as_deref(),
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "duplicate-file-groups",
            &signature,
        )?;
        validate_cursor_value(
            cursor.as_ref(),
            sort_field == DuplicateFileGroupSortField::RepresentativeName,
        )?;
        let query_started = Instant::now();
        let page = db
            .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
                run_id: parameters.run_id,
                limit: parameters.page_size,
                sort_field,
                sort_direction,
                filter: DuplicateFileGroupFilter {
                    search,
                    path_match,
                    extension_key: extension,
                    extension_match,
                    minimum_size,
                    minimum_copy_count: parameters.filter.minimum_copy_count,
                    across_drives: parameters.filter.across_drives,
                    selected_root,
                    selected_drive,
                },
                cursor: cursor.clone(),
            })
            .map_err(internal_database_error)?;
        log_result_query(
            "duplicate_file_group.page",
            parameters.run_id,
            None,
            parameters.page_size,
            page.groups.len(),
            page.total,
            query_started.elapsed(),
        );
        let previous_cursor = page
            .groups
            .first()
            .and_then(|group| {
                let has_previous =
                    cursor.as_ref().map_or(false, |value| !value.before) || page.has_more;
                has_previous.then(|| encode_group_cursor(group, sort_field, true, &signature))
            })
            .transpose()?;
        let next_cursor = page
            .groups
            .last()
            .and_then(|group| {
                let has_next = cursor.as_ref().is_some_and(|value| value.before) || page.has_more;
                has_next.then(|| encode_group_cursor(group, sort_field, false, &signature))
            })
            .transpose()?;
        let summary = review_summary_dto(&page.summary);
        let groups = page.groups.into_iter().map(group_dto).collect::<Vec<_>>();
        Ok(json!({
            "groups": groups,
            "total": page.total,
            "summary": summary,
            "nextCursor": next_cursor,
            "previousCursor": previous_cursor,
        }))
    }

    fn duplicate_file_selected_root_facet_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: DuplicateFileSelectedRootFacetPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        ensure_completed_result_run(&db, parameters.run_id)?;
        let sort_field = parse_selected_root_facet_sort_field(&parameters.sort.field)?;
        let sort_direction = parse_sort_direction(&parameters.sort.direction)?;
        let requested_path_match = parse_path_match(&parameters.filter.path_match)?;
        let search = validate_group_path_filter(parameters.filter.search, requested_path_match)?;
        let path_match = search
            .as_ref()
            .map_or(DuplicateFilePathMatchMode::Substring, |_| {
                requested_path_match
            });
        let extension = normalize_extension_filter(parameters.filter.extension.as_deref())?;
        let extension_match =
            normalize_extension_match(&parameters.filter.extension_match, extension.as_deref())?;
        let minimum_size =
            parse_non_negative_decimal(&parameters.filter.minimum_size, "filter.minimumSize")?;
        validate_minimum_copy_count(parameters.filter.minimum_copy_count)?;
        let selected_drive = parameters
            .filter
            .selected_drive
            .filter(|value| !value.is_empty());
        let signature = selected_root_facet_query_signature(
            parameters.run_id,
            sort_field,
            sort_direction,
            search.as_deref(),
            path_match,
            extension.as_deref(),
            extension_match,
            minimum_size,
            parameters.filter.minimum_copy_count,
            parameters.filter.across_drives,
            selected_drive.as_deref(),
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "duplicate-file-selected-root-facets",
            &signature,
        )?;
        validate_cursor_value(
            cursor.as_ref(),
            sort_field == DuplicateFileSelectedRootFacetSortField::Value,
        )?;
        let query_started = Instant::now();
        let page = db
            .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
                run_id: parameters.run_id,
                limit: parameters.page_size,
                sort_field,
                sort_direction,
                filter: DuplicateFileGroupFilter {
                    search,
                    path_match,
                    extension_key: extension,
                    extension_match,
                    minimum_size,
                    minimum_copy_count: parameters.filter.minimum_copy_count,
                    across_drives: parameters.filter.across_drives,
                    selected_root: None,
                    selected_drive,
                },
                cursor: cursor.clone(),
            })
            .map_err(internal_database_error)?;
        log_result_query(
            "duplicate_file_selected_root_facet.page",
            parameters.run_id,
            None,
            parameters.page_size,
            page.facets.len(),
            page.total,
            query_started.elapsed(),
        );
        let previous_cursor = page
            .facets
            .first()
            .and_then(|facet| {
                let has_previous =
                    cursor.as_ref().map_or(false, |value| !value.before) || page.has_more;
                has_previous
                    .then(|| encode_selected_root_facet_cursor(facet, sort_field, true, &signature))
            })
            .transpose()?;
        let next_cursor = page
            .facets
            .last()
            .and_then(|facet| {
                let has_next = cursor.as_ref().is_some_and(|value| value.before) || page.has_more;
                has_next.then(|| {
                    encode_selected_root_facet_cursor(facet, sort_field, false, &signature)
                })
            })
            .transpose()?;
        let facets = page
            .facets
            .into_iter()
            .map(selected_root_facet_dto)
            .collect::<Vec<_>>();
        Ok(json!({
            "facets": facets,
            "total": page.total,
            "nextCursor": next_cursor,
            "previousCursor": previous_cursor,
        }))
    }

    fn duplicate_file_drive_facet_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: DuplicateFileDriveFacetPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        ensure_completed_result_run(&db, parameters.run_id)?;
        let sort_field = parse_drive_facet_sort_field(&parameters.sort.field)?;
        let sort_direction = parse_sort_direction(&parameters.sort.direction)?;
        let requested_path_match = parse_path_match(&parameters.filter.path_match)?;
        let search = validate_group_path_filter(parameters.filter.search, requested_path_match)?;
        let path_match = search
            .as_ref()
            .map_or(DuplicateFilePathMatchMode::Substring, |_| {
                requested_path_match
            });
        let extension = normalize_extension_filter(parameters.filter.extension.as_deref())?;
        let extension_match =
            normalize_extension_match(&parameters.filter.extension_match, extension.as_deref())?;
        let minimum_size =
            parse_non_negative_decimal(&parameters.filter.minimum_size, "filter.minimumSize")?;
        validate_minimum_copy_count(parameters.filter.minimum_copy_count)?;
        let selected_root = parameters
            .filter
            .selected_root
            .filter(|value| !value.is_empty());
        let signature = drive_facet_query_signature(
            parameters.run_id,
            sort_field,
            sort_direction,
            search.as_deref(),
            path_match,
            extension.as_deref(),
            extension_match,
            minimum_size,
            parameters.filter.minimum_copy_count,
            parameters.filter.across_drives,
            selected_root.as_deref(),
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "duplicate-file-drive-facets",
            &signature,
        )?;
        validate_cursor_value(
            cursor.as_ref(),
            sort_field == DuplicateFileDriveFacetSortField::Value,
        )?;
        let query_started = Instant::now();
        let page = db
            .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
                run_id: parameters.run_id,
                limit: parameters.page_size,
                sort_field,
                sort_direction,
                filter: DuplicateFileGroupFilter {
                    search,
                    path_match,
                    extension_key: extension,
                    extension_match,
                    minimum_size,
                    minimum_copy_count: parameters.filter.minimum_copy_count,
                    across_drives: parameters.filter.across_drives,
                    selected_root,
                    selected_drive: None,
                },
                cursor: cursor.clone(),
            })
            .map_err(internal_database_error)?;
        log_result_query(
            "duplicate_file_drive_facet.page",
            parameters.run_id,
            None,
            parameters.page_size,
            page.facets.len(),
            page.total,
            query_started.elapsed(),
        );
        let previous_cursor = page
            .facets
            .first()
            .and_then(|facet| {
                let has_previous =
                    cursor.as_ref().map_or(false, |value| !value.before) || page.has_more;
                has_previous.then(|| encode_drive_facet_cursor(facet, sort_field, true, &signature))
            })
            .transpose()?;
        let next_cursor = page
            .facets
            .last()
            .and_then(|facet| {
                let has_next = cursor.as_ref().is_some_and(|value| value.before) || page.has_more;
                has_next.then(|| encode_drive_facet_cursor(facet, sort_field, false, &signature))
            })
            .transpose()?;
        let facets = page
            .facets
            .into_iter()
            .map(drive_facet_dto)
            .collect::<Vec<_>>();
        Ok(json!({
            "facets": facets,
            "total": page.total,
            "nextCursor": next_cursor,
            "previousCursor": previous_cursor,
        }))
    }

    fn duplicate_file_group_members(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: DuplicateFileMemberPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        ensure_completed_result_run(&db, parameters.run_id)?;
        if !db
            .duplicate_file_group_exists(parameters.run_id, parameters.group_id)
            .map_err(internal_database_error)?
        {
            return Err(ProtocolFailure::new(
                "duplicate_group_not_found",
                format!(
                    "Duplicate file group {} was not found in run {}",
                    parameters.group_id, parameters.run_id
                ),
            )
            .with_details(json!({
                "runId": parameters.run_id,
                "groupId": parameters.group_id,
            })));
        }
        let sort_field = parse_member_sort_field(&parameters.sort.field)?;
        let sort_direction = parse_sort_direction(&parameters.sort.direction)?;
        let search = validate_search(parameters.filter.search)?;
        let signature = member_query_signature(
            parameters.run_id,
            parameters.group_id,
            sort_field,
            sort_direction,
            search.as_deref(),
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "duplicate-file-members",
            &signature,
        )?;
        validate_cursor_value(
            cursor.as_ref(),
            sort_field == DuplicateFileMemberSortField::Path,
        )?;
        let query_started = Instant::now();
        let page = db
            .page_duplicate_file_members(&DuplicateFileMemberPageQuery {
                run_id: parameters.run_id,
                group_id: parameters.group_id,
                limit: parameters.page_size,
                sort_field,
                sort_direction,
                filter: DuplicateFileMemberFilter { search },
                cursor: cursor.clone(),
            })
            .map_err(internal_database_error)?;
        log_result_query(
            "duplicate_file_group.members",
            parameters.run_id,
            Some(parameters.group_id),
            parameters.page_size,
            page.members.len(),
            page.total,
            query_started.elapsed(),
        );
        let previous_cursor = page
            .members
            .first()
            .and_then(|member| {
                let has_previous =
                    cursor.as_ref().map_or(false, |value| !value.before) || page.has_more;
                has_previous.then(|| encode_member_cursor(member, sort_field, true, &signature))
            })
            .transpose()?;
        let next_cursor = page
            .members
            .last()
            .and_then(|member| {
                let has_next = cursor.as_ref().is_some_and(|value| value.before) || page.has_more;
                has_next.then(|| encode_member_cursor(member, sort_field, false, &signature))
            })
            .transpose()?;
        let members = page.members.into_iter().map(member_dto).collect::<Vec<_>>();
        Ok(json!({
            "members": members,
            "total": page.total,
            "nextCursor": next_cursor,
            "previousCursor": previous_cursor,
            "reviewPlanId": page.review_plan_id,
            "reviewRevision": page.review_revision,
            "reviewSummary": review_group_summary_dto(&page.review_summary),
        }))
    }

    fn duplicate_folder_group_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: DuplicateFolderGroupPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        ensure_completed_result_run(&db, parameters.run_id)?;
        let sort_field = parse_folder_group_sort_field(&parameters.sort.field)?;
        let sort_direction = parse_sort_direction(&parameters.sort.direction)?;
        let search = validate_search(parameters.filter.search)?;
        let minimum_size =
            parse_non_negative_decimal(&parameters.filter.minimum_size, "filter.minimumSize")?;
        let signature = folder_group_query_signature(
            parameters.run_id,
            sort_field,
            sort_direction,
            search.as_deref(),
            minimum_size,
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "duplicate-folder-groups",
            &signature,
        )?;
        validate_cursor_value(
            cursor.as_ref(),
            sort_field == DuplicateFolderGroupSortField::RepresentativePath,
        )?;
        let query_started = Instant::now();
        let page = db
            .page_duplicate_folder_groups(&DuplicateFolderGroupPageQuery {
                run_id: parameters.run_id,
                limit: parameters.page_size,
                sort_field,
                sort_direction,
                filter: DuplicateFolderGroupFilter {
                    search,
                    minimum_size,
                },
                cursor: cursor.clone(),
            })
            .map_err(internal_database_error)?;
        log_result_query(
            "duplicate_folder_group.page",
            parameters.run_id,
            None,
            parameters.page_size,
            page.groups.len(),
            page.total,
            query_started.elapsed(),
        );
        let previous_cursor = page
            .groups
            .first()
            .and_then(|group| {
                let has_previous = cursor
                    .as_ref()
                    .is_some_and(|value| !value.before || page.has_more);
                has_previous
                    .then(|| encode_folder_group_cursor(group, sort_field, true, &signature))
            })
            .transpose()?;
        let next_cursor = page
            .groups
            .last()
            .and_then(|group| {
                let has_next = cursor.as_ref().is_some_and(|value| value.before) || page.has_more;
                has_next.then(|| encode_folder_group_cursor(group, sort_field, false, &signature))
            })
            .transpose()?;
        Ok(json!({
            "groups": page.groups.into_iter().map(folder_group_dto).collect::<Vec<_>>(),
            "total": page.total,
            "nextCursor": next_cursor,
            "previousCursor": previous_cursor,
        }))
    }

    fn duplicate_folder_group_members(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: DuplicateFolderMemberPageParameters = parse_parameters(request)?;
        validate_result_page_size(parameters.page_size)?;
        let db = self.state.database()?;
        ensure_completed_result_run(&db, parameters.run_id)?;
        if !db
            .duplicate_folder_group_exists(parameters.run_id, parameters.group_id)
            .map_err(internal_database_error)?
        {
            return Err(ProtocolFailure::new(
                "duplicate_folder_group_not_found",
                format!(
                    "Duplicate folder group {} was not found in run {}",
                    parameters.group_id, parameters.run_id
                ),
            )
            .with_details(json!({
                "runId": parameters.run_id,
                "groupId": parameters.group_id,
            })));
        }
        let sort_field = parse_folder_member_sort_field(&parameters.sort.field)?;
        let sort_direction = parse_sort_direction(&parameters.sort.direction)?;
        let search = validate_search(parameters.filter.search)?;
        let review = db
            .get_review_plan_view(parameters.run_id)
            .map_err(review_error)?;
        let review_plan_id = review.plan.as_ref().map_or(0, |value| value.id);
        let review_revision = review.plan.as_ref().map_or(0, |value| value.revision);
        let signature = folder_member_query_signature(
            parameters.run_id,
            parameters.group_id,
            sort_field,
            sort_direction,
            search.as_deref(),
            review_plan_id,
            review_revision,
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "duplicate-folder-members",
            &signature,
        )?;
        validate_cursor_value(cursor.as_ref(), true)?;
        let query_started = Instant::now();
        let page = db
            .page_duplicate_folder_members(&DuplicateFolderMemberPageQuery {
                run_id: parameters.run_id,
                group_id: parameters.group_id,
                limit: parameters.page_size,
                sort_field,
                sort_direction,
                filter: DuplicateFolderMemberFilter { search },
                cursor: cursor.clone(),
            })
            .map_err(internal_database_error)?;
        log_result_query(
            "duplicate_folder_group.members",
            parameters.run_id,
            Some(parameters.group_id),
            parameters.page_size,
            page.members.len(),
            page.total,
            query_started.elapsed(),
        );
        let previous_cursor = page
            .members
            .first()
            .and_then(|member| {
                let has_previous = cursor
                    .as_ref()
                    .is_some_and(|value| !value.before || page.has_more);
                has_previous.then(|| encode_folder_member_cursor(member, true, &signature))
            })
            .transpose()?;
        let next_cursor = page
            .members
            .last()
            .and_then(|member| {
                let has_next = cursor.as_ref().is_some_and(|value| value.before) || page.has_more;
                has_next.then(|| encode_folder_member_cursor(member, false, &signature))
            })
            .transpose()?;
        Ok(json!({
            "members": page.members.into_iter().map(folder_member_dto).collect::<Vec<_>>(),
            "total": page.total,
            "nextCursor": next_cursor,
            "previousCursor": previous_cursor,
            "reviewPlanId": page.review_plan_id,
            "reviewRevision": page.review_revision,
            "reviewSummary": review_folder_group_summary_dto(&page.review_summary),
        }))
    }

    fn preflight_start(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: PreflightStartParameters = parse_parameters(request)?;
        if parameters.operation_id.trim().is_empty()
            || parameters.operation_id.chars().count() > MAXIMUM_OPERATION_ID_CHARACTERS
            || parameters.run_id <= 0
            || parameters.expected_review_revision < 0
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "preflight.start requires a positive runId, a non-negative expectedReviewRevision, and a 1..=128 character operationId",
            ));
        }
        let db = self.state.database()?;
        if let Some(existing) = db
            .get_preflight_by_operation(&parameters.operation_id)
            .map_err(preflight_error)?
        {
            if existing.preflight.run_id != parameters.run_id
                || existing.preflight.review_revision != parameters.expected_review_revision
            {
                return Err(preflight_error(PreflightError::IdempotencyConflict {
                    operation_id: parameters.operation_id,
                }));
            }
            return Ok(json!({
                "preflight":preflight_view_dto(&existing),
                "replayed":true,
            }));
        }
        let _gate = self
            .state
            .work_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(run_id) = self.state.active_run_id() {
            return Err(ProtocolFailure {
                code: "preflight_busy",
                message: "A scan is already using the filesystem".to_owned(),
                retryable: true,
                details: json!({"activeRunId":run_id}),
            });
        }
        if let Some(preflight_id) = self.state.active_preflight_id() {
            return Err(ProtocolFailure {
                code: "preflight_busy",
                message: "A preflight is already running".to_owned(),
                retryable: true,
                details: json!({"activePreflightId":preflight_id}),
            });
        }
        let result = db
            .create_preflight(
                &parameters.operation_id,
                parameters.run_id,
                parameters.expected_review_revision,
            )
            .map_err(preflight_error)?;
        if result.view.preflight.status != "pending" {
            return Ok(json!({
                "preflight": preflight_view_dto(&result.view),
                "replayed": result.replayed,
            }));
        }

        let started = db
            .mark_preflight_running(result.view.preflight.id)
            .map_err(preflight_error)?;
        let cancel_token = Arc::new(AtomicBool::new(false));
        *self
            .state
            .active_preflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ActivePreflight {
            preflight_id: started.id,
            cancel_token: cancel_token.clone(),
        });
        let view = db.get_preflight_view(started.id).map_err(preflight_error)?;
        self.state.emit(
            "preflight.started",
            &json!({"preflight":preflight_view_dto(&view)}),
        );
        let state = self.state.clone();
        std::thread::spawn(move || run_preflight_thread(state, started.id, cancel_token));
        Ok(json!({
            "preflight": preflight_view_dto(&view),
            "replayed": result.replayed,
        }))
    }

    fn preflight_get(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: PreflightGetParameters = parse_parameters(request)?;
        let db = self.state.database()?;
        let view = match (parameters.preflight_id, parameters.run_id) {
            (Some(preflight_id), None) if preflight_id > 0 => Some(
                db.get_preflight_view(preflight_id)
                    .map_err(preflight_error)?,
            ),
            (None, Some(run_id)) if run_id > 0 => db
                .latest_preflight_for_run(run_id)
                .map_err(preflight_error)?,
            _ => {
                return Err(ProtocolFailure::new(
                    "invalid_request",
                    "preflight.get requires exactly one positive preflightId or runId",
                ))
            }
        };
        Ok(json!({"preflight":view.as_ref().map(preflight_view_dto)}))
    }

    fn preflight_item_page(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: PreflightItemPageParameters = parse_parameters(request)?;
        if parameters.preflight_id <= 0 || !(1..=200).contains(&parameters.page_size) {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "preflight.item.page requires a positive preflightId and pageSize 1..=200",
            ));
        }
        let signature = format!(
            "{}|{}",
            parameters.preflight_id,
            parameters.outcome.as_deref().unwrap_or("all")
        );
        let cursor = decode_cursor(parameters.cursor.as_deref(), "preflight-items", &signature)?;
        validate_cursor_value(cursor.as_ref(), false)?;
        if cursor
            .as_ref()
            .is_some_and(|cursor| cursor.id != parameters.preflight_id || cursor.before)
        {
            return Err(invalid_cursor());
        }
        let offset = cursor
            .as_ref()
            .and_then(|cursor| match cursor.value {
                PageCursorValue::Integer(value) => Some(value),
                PageCursorValue::Text(_) => None,
            })
            .unwrap_or(0);
        let db = self.state.database()?;
        let preflight = db
            .get_preflight_view(parameters.preflight_id)
            .map_err(preflight_error)?;
        let query_started = Instant::now();
        let page = db
            .page_preflight_items(
                parameters.preflight_id,
                offset,
                parameters.page_size,
                parameters.outcome.as_deref(),
            )
            .map_err(preflight_error)?;
        log_result_query(
            "preflight.item.page",
            preflight.preflight.run_id,
            None,
            i64::from(parameters.page_size),
            page.items.len(),
            page.total,
            query_started.elapsed(),
        );
        let next_cursor = if page.has_more {
            Some(encode_cursor(CursorPayload {
                version: 1,
                kind: "preflight-items".to_owned(),
                query: signature,
                before: false,
                value: CursorScalar::Integer((offset + page.items.len() as i64).to_string()),
                id: parameters.preflight_id,
            })?)
        } else {
            None
        };
        Ok(json!({
            "items":page.items.iter().map(preflight_item_dto).collect::<Vec<_>>(),
            "total":page.total,
            "nextCursor":next_cursor,
        }))
    }

    fn preflight_cancel(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: PreflightCancelParameters = parse_parameters(request)?;
        if parameters.preflight_id <= 0 {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "preflight.cancel requires a positive preflightId",
            ));
        }
        let active = self
            .state
            .active_preflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(current) = active.as_ref() {
            if current.preflight_id != parameters.preflight_id {
                return Err(ProtocolFailure::new(
                    "preflight_not_cancellable",
                    "The requested preflight is not the active validation",
                )
                .with_details(json!({"activePreflightId":current.preflight_id})));
            }
            let db = self.state.database()?;
            db.mark_preflight_cancelling(parameters.preflight_id)
                .map_err(preflight_error)?;
            current.cancel_token.store(true, Ordering::Release);
            let view = db
                .get_preflight_view(parameters.preflight_id)
                .map_err(preflight_error)?;
            return Ok(json!({"preflight":preflight_view_dto(&view)}));
        }
        let view = self
            .state
            .database()?
            .get_preflight_view(parameters.preflight_id)
            .map_err(preflight_error)?;
        if matches!(view.preflight.status.as_str(), "cancelled" | "cancelling") {
            Ok(json!({"preflight":preflight_view_dto(&view)}))
        } else {
            Err(ProtocolFailure::new(
                "preflight_not_cancellable",
                format!("Preflight is already {}", view.preflight.status),
            )
            .with_details(json!({
                "preflightId":parameters.preflight_id,
                "status":view.preflight.status,
            })))
        }
    }

    fn recycle_operation_prepare(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleOperationPrepareParameters = parse_parameters(request)?;
        if self.state.active_run_id().is_some() || self.state.active_preflight_id().is_some() {
            return Err(ProtocolFailure::new(
                "operation_busy",
                "A scan or preflight is still using the filesystem",
            )
            .with_details(json!({
                "activeRunId":self.state.active_run_id(),
                "activePreflightId":self.state.active_preflight_id(),
            })));
        }
        let result = self
            .state
            .database()?
            .prepare_recycle_operation(
                &parameters.operation_id,
                parameters.run_id,
                parameters.preflight_id,
                parameters.expected_review_revision,
            )
            .map_err(recycle_operation_error)?;
        Ok(json!({
            "operation": recycle_operation_view_dto(&result.view),
            "replayed": result.replayed,
            "executorEnabled": false,
        }))
    }

    fn recycle_operation_get(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleOperationGetParameters = parse_parameters(request)?;
        let db = self.state.database()?;
        let operation = match (parameters.recycle_operation_id, parameters.run_id) {
            (Some(operation_id), None) if operation_id > 0 => Some(
                db.get_recycle_operation(operation_id)
                    .map_err(recycle_operation_error)?,
            ),
            (None, Some(run_id)) if run_id > 0 => db
                .latest_recycle_operation_for_run(run_id)
                .map_err(recycle_operation_error)?,
            _ => return Err(ProtocolFailure::new(
                "invalid_request",
                "recycle_operation.get requires exactly one positive recycleOperationId or runId",
            )),
        };
        Ok(json!({
            "operation": operation.as_ref().map(recycle_operation_view_dto),
            "executorEnabled": false,
        }))
    }

    fn recycle_operation_item_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleOperationItemPageParameters = parse_parameters(request)?;
        if parameters.recycle_operation_id <= 0 || !(1..=200).contains(&parameters.page_size) {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "recycle_operation.item.page requires a positive recycleOperationId and pageSize 1..=200",
            ));
        }
        let signature = format!(
            "{}|{}",
            parameters.recycle_operation_id,
            parameters.result_status.as_deref().unwrap_or("all")
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "recycle-operation-items",
            &signature,
        )?;
        validate_cursor_value(cursor.as_ref(), false)?;
        if cursor
            .as_ref()
            .is_some_and(|cursor| cursor.id != parameters.recycle_operation_id || cursor.before)
        {
            return Err(invalid_cursor());
        }
        let offset = cursor
            .as_ref()
            .and_then(|cursor| match cursor.value {
                PageCursorValue::Integer(value) => Some(value),
                PageCursorValue::Text(_) => None,
            })
            .unwrap_or(0);
        let db = self.state.database()?;
        let operation = db
            .get_recycle_operation(parameters.recycle_operation_id)
            .map_err(recycle_operation_error)?;
        let query_started = Instant::now();
        let page = db
            .page_recycle_operation_items(
                parameters.recycle_operation_id,
                offset,
                parameters.page_size,
                parameters.result_status.as_deref(),
            )
            .map_err(recycle_operation_error)?;
        log_result_query(
            "recycle_operation.item.page",
            operation.operation.run_id,
            None,
            parameters.page_size,
            page.items.len(),
            page.total,
            query_started.elapsed(),
        );
        let next_cursor = if page.has_more {
            Some(encode_cursor(CursorPayload {
                version: 1,
                kind: "recycle-operation-items".to_owned(),
                query: signature,
                before: false,
                value: CursorScalar::Integer((offset + page.items.len() as i64).to_string()),
                id: parameters.recycle_operation_id,
            })?)
        } else {
            None
        };
        Ok(json!({
            "items": page.items.iter().map(recycle_operation_item_dto).collect::<Vec<_>>(),
            "total": page.total,
            "nextCursor": next_cursor,
        }))
    }

    fn recycle_operation_eligibility_report(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleEligibilityReportParameters = parse_parameters(request)?;
        let observations = parameters
            .items
            .into_iter()
            .map(|item| RecycleEligibilityObservation {
                item_id: item.item_id,
                status: item.status,
                reason_code: item.reason_code,
            })
            .collect::<Vec<_>>();
        let result = self
            .state
            .database()?
            .report_recycle_eligibility(
                &parameters.report_operation_id,
                parameters.recycle_operation_id,
                &observations,
            )
            .map_err(recycle_operation_error)?;
        Ok(json!({
            "operation": recycle_operation_view_dto(&result.view),
            "replayed": result.replayed,
            "executorEnabled": false,
        }))
    }

    fn recycle_operation_confirm(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleOperationConfirmParameters = parse_parameters(request)?;
        let result = self
            .state
            .database()?
            .confirm_recycle_operation(
                &parameters.report_operation_id,
                parameters.recycle_operation_id,
                &parameters.confirmation_signature,
            )
            .map_err(recycle_operation_error)?;
        Ok(json!({
            "operation": recycle_operation_view_dto(&result.view),
            "replayed": result.replayed,
            "executorEnabled": false,
        }))
    }

    fn recycle_operation_cancel(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleOperationIdParameters = parse_parameters(request)?;
        let view = self
            .state
            .database()?
            .cancel_recycle_operation(parameters.recycle_operation_id)
            .map_err(recycle_operation_error)?;
        Ok(json!({
            "operation": recycle_operation_view_dto(&view),
            "executorEnabled": false,
        }))
    }

    fn recycle_operation_batch_next(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleOperationIdParameters = parse_parameters(request)?;
        let batch = self
            .state
            .database()?
            .next_recycle_operation_batch(parameters.recycle_operation_id)
            .map_err(recycle_operation_error)?;
        Ok(json!({
            "batch": batch.as_ref().map(recycle_operation_batch_dto),
            "executorEnabled": false,
        }))
    }

    fn recycle_operation_batch_begin(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleBatchBeginParameters = parse_parameters(request)?;
        let result = self
            .state
            .database()?
            .begin_recycle_operation_batch(
                &parameters.report_operation_id,
                parameters.recycle_operation_id,
                parameters.batch_id,
                &parameters.shell_attempt_id,
            )
            .map_err(recycle_operation_error)?;
        Ok(json!({
            "operation": recycle_operation_view_dto(&result.view),
            "replayed": result.replayed,
            "executorEnabled": false,
        }))
    }

    fn recycle_operation_batch_report(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecycleBatchReportParameters = parse_parameters(request)?;
        let observations = parameters
            .items
            .into_iter()
            .map(|item| RecycleItemResultObservation {
                item_id: item.item_id,
                status: item.status,
                reason_code: item.reason_code,
                shell_hresult: item.shell_hresult,
                recycled_item_present: item.recycled_item_present,
            })
            .collect::<Vec<_>>();
        let result = self
            .state
            .database()?
            .report_recycle_operation_batch(
                &parameters.report_operation_id,
                parameters.recycle_operation_id,
                parameters.batch_id,
                &observations,
            )
            .map_err(recycle_operation_error)?;
        Ok(json!({
            "operation": recycle_operation_view_dto(&result.view),
            "replayed": result.replayed,
            "executorEnabled": false,
        }))
    }

    fn recovery_review_get(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: RecoveryReviewGetParameters = parse_parameters(request)?;
        let review = self
            .state
            .database()?
            .get_recovery_review(parameters.recycle_operation_id)
            .map_err(recovery_review_error)?;
        Ok(json!({
            "review": recovery_review_summary_dto(&review),
            "executorEnabled": false,
        }))
    }

    fn recovery_review_observation_page(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecoveryReviewObservationPageParameters = parse_parameters(request)?;
        if parameters.recycle_operation_id <= 0 || !(1..=200).contains(&parameters.page_size) {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "recovery_review.observation.page requires a positive recycleOperationId and pageSize 1..=200",
            ));
        }
        let signature = format!(
            "{}|{}",
            parameters.recycle_operation_id, parameters.current_only
        );
        let cursor = decode_cursor(
            parameters.cursor.as_deref(),
            "recovery-review-observations",
            &signature,
        )?;
        validate_cursor_value(cursor.as_ref(), false)?;
        if cursor
            .as_ref()
            .is_some_and(|cursor| cursor.id != parameters.recycle_operation_id || cursor.before)
        {
            return Err(invalid_cursor());
        }
        let offset = cursor
            .as_ref()
            .and_then(|cursor| match cursor.value {
                PageCursorValue::Integer(value) => Some(value),
                PageCursorValue::Text(_) => None,
            })
            .unwrap_or(0);
        let db = self.state.database()?;
        let operation = db
            .get_recycle_operation(parameters.recycle_operation_id)
            .map_err(recycle_operation_error)?;
        let query_started = Instant::now();
        let page = db
            .page_recovery_review_observations(
                parameters.recycle_operation_id,
                offset,
                parameters.page_size,
                parameters.current_only,
            )
            .map_err(recovery_review_error)?;
        log_result_query(
            "recovery_review.observation.page",
            operation.operation.run_id,
            None,
            parameters.page_size,
            page.observations.len(),
            page.total,
            query_started.elapsed(),
        );
        let next_cursor = if page.has_more {
            Some(encode_cursor(CursorPayload {
                version: 1,
                kind: "recovery-review-observations".to_owned(),
                query: signature,
                before: false,
                value: CursorScalar::Integer((offset + page.observations.len() as i64).to_string()),
                id: parameters.recycle_operation_id,
            })?)
        } else {
            None
        };
        Ok(json!({
            "observations": page.observations.iter().map(recovery_review_observation_dto).collect::<Vec<_>>(),
            "total": page.total,
            "nextCursor": next_cursor,
            "executorEnabled": false,
        }))
    }

    fn recovery_review_observation_record(
        &self,
        request: &RequestEnvelope,
    ) -> Result<Value, ProtocolFailure> {
        let parameters: RecoveryReviewObservationRecordParameters = parse_parameters(request)?;
        let observation = RecoveryObservationKind::parse(&parameters.observation).ok_or_else(|| {
            ProtocolFailure::new(
                "invalid_request",
                "observation must be observed_in_recycle_bin, observed_at_source, observed_in_both, observed_in_neither, or deferred_unresolved",
            )
        })?;
        let result = self
            .state
            .database()?
            .record_recovery_review_observation(&RecoveryReviewObservationInput {
                request_id: parameters.request_id,
                recycle_operation_id: parameters.recycle_operation_id,
                item_id: parameters.item_id,
                observation,
                observed_at: parameters.observed_at,
                note: parameters.note,
                evidence_version: parameters.evidence_version,
                supersedes_observation_id: parameters.supersedes_observation_id,
                correction_reason: parameters.correction_reason,
            })
            .map_err(recovery_review_error)?;
        Ok(json!({
            "review": recovery_review_summary_dto(&result.summary),
            "observation": recovery_review_observation_dto(&result.observation),
            "replayed": result.replayed,
            "executorEnabled": false,
        }))
    }

    fn run_start(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: IdParameters = parse_parameters(request)?;
        let _gate = self
            .state
            .work_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(preflight_id) = self.state.active_preflight_id() {
            return Err(ProtocolFailure {
                code: "scan_busy",
                message: "A preflight is already using the filesystem".to_owned(),
                retryable: true,
                details: json!({"activePreflightId":preflight_id}),
            });
        }
        let mut active = self
            .state
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(current) = active.as_ref() {
            return Err(ProtocolFailure {
                code: "scan_busy",
                message: "A scan is already running".to_owned(),
                retryable: true,
                details: json!({"activeRunId":current.run_id}),
            });
        }

        let db = self.state.database()?;
        let session = get_session(&db, parameters.session_id)?;
        let session = session_dto(session)?;
        if session.cloud_policy != CloudPolicy::ExcludeRegisteredRoots {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "This version enables only the exclude_registered_roots cloud policy",
            )
            .with_details(json!({"sessionId":parameters.session_id, "field":"cloudPolicy"})));
        }
        if session.cloud_detection_status != CloudDetectionStatus::Complete {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "Registered cloud location detection must complete before starting this scan",
            )
            .with_details(json!({
                "sessionId":parameters.session_id,
                "field":"cloudDetectionStatus",
                "status":session.cloud_detection_status.as_str(),
            })));
        }
        let effective_exclusions = session
            .manual_location_exclusions
            .iter()
            .chain(
                session
                    .registered_cloud_locations
                    .iter()
                    .map(|location| &location.path),
            )
            .collect::<Vec<_>>();
        if !session.roots.iter().any(|root| {
            effective_exclusions
                .iter()
                .any(|exclusion| path_is_within(Path::new(root), Path::new(exclusion)))
                || Path::new(root).is_dir()
        }) {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "At least one session root must be an accessible directory",
            )
            .with_details(json!({"sessionId":parameters.session_id, "field":"roots"})));
        }
        let run_parameters = RunParameters {
            roots: session.roots.clone(),
            ignore_patterns: session.ignore_patterns.clone(),
            directory_similarity_threshold_millis: 500,
            cloud_policy: session.cloud_policy,
            manual_location_exclusions: session.manual_location_exclusions.clone(),
            registered_cloud_locations: session.registered_cloud_locations.clone(),
            cloud_detection_status: session.cloud_detection_status,
        };
        let run_id = db
            .create_scan_run(
                parameters.session_id,
                &run_parameters,
                super_duper_core::ENGINE_VERSION,
            )
            .map_err(internal_database_error)?;
        if let Err(error) = db.start_scan_run(run_id) {
            let _ = db.fail_scan_run(run_id, &error.to_string());
            return Err(internal_database_error(error));
        }

        let engine = ScanEngine::new(AppConfig {
            root_paths: run_parameters.roots.clone(),
            ignore_patterns: run_parameters.ignore_patterns.clone(),
        })
        .with_db_path(&self.state.database_path.to_string_lossy())
        .with_status_db_path(&self.state.status_database_path.to_string_lossy())
        .with_status_worker_version(env!("CARGO_PKG_VERSION"))
        .with_session_id(parameters.session_id);
        let cancel_token = engine.cancel_token();
        *active = Some(ActiveRun {
            run_id,
            cancel_token: cancel_token.clone(),
        });
        drop(active);

        let started = run_dto(get_run(&db, run_id)?)?;
        self.state.emit("run.started", &json!({"run":started}));
        let state = self.state.clone();
        std::thread::spawn(move || run_scan_thread(state, engine, run_id, cancel_token));
        Ok(json!({"run":started}))
    }

    fn run_cancel(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: RunIdParameters = parse_parameters(request)?;
        let active = self
            .state
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(current) = active.as_ref() else {
            let run = get_run(&self.state.database()?, parameters.run_id)?;
            return Err(ProtocolFailure::new(
                "invalid_state",
                format!("Run {} is already {}", run.id, run.status),
            )
            .with_details(json!({"runId":run.id, "status":run.status})));
        };
        if current.run_id != parameters.run_id {
            return Err(ProtocolFailure::new(
                "invalid_state",
                "The requested run is not the active scan",
            )
            .with_details(json!({"activeRunId":current.run_id})));
        }
        let db = self.state.database()?;
        match db.mark_run_cancelling(parameters.run_id) {
            Ok(()) => {}
            Err(_) => {
                let durable = get_run(&db, parameters.run_id)?;
                if durable.status != "cancelling" {
                    return Err(ProtocolFailure::new(
                        "invalid_state",
                        format!("Run {} is already {}", durable.id, durable.status),
                    )
                    .with_details(json!({"runId":durable.id, "status":durable.status})));
                }
            }
        }
        let run = run_dto(get_run(&db, parameters.run_id)?)?;
        // Publish cancellation only after the durable state and response snapshot are
        // `cancelling`. Otherwise the scan thread can win either race and make this initiating
        // request fail or return a terminal state even though it caused the cancellation.
        current.cancel_token.store(true, Ordering::Release);
        Ok(json!({"run":run}))
    }
}

fn run_preflight_thread(state: Arc<SharedState>, preflight_id: i64, cancel_token: Arc<AtomicBool>) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let db = Database::open_connection(&state.database_path.to_string_lossy())?;
        let mut last_event = None::<Instant>;
        db.validate_preflight(preflight_id, &cancel_token, |preflight, current_path| {
            let now = Instant::now();
            let terminal = matches!(
                preflight.status.as_str(),
                "completed" | "cancelled" | "failed" | "interrupted"
            );
            if terminal
                || last_event.map_or(true, |last| now.duration_since(last) >= EVENT_INTERVAL)
            {
                last_event = Some(now);
                state.emit(
                    "preflight.progress",
                    &json!({
                        "preflightId":preflight.id,
                        "status":preflight.status,
                        "processedItemCount":preflight.summary.processed_item_count,
                        "totalItemCount":preflight.summary.total_item_count,
                        "readyCount":preflight.summary.ready_count,
                        "changedCount":preflight.summary.changed_count,
                        "missingCount":preflight.summary.missing_count,
                        "unavailableCount":preflight.summary.unavailable_count,
                        "conflictCount":preflight.summary.conflict_count,
                        "currentPath":current_path,
                    }),
                );
            }
        })
    }));
    match outcome {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            if let Ok(db) = Database::open_connection(&state.database_path.to_string_lossy()) {
                let _ = db.fail_preflight(preflight_id, "validation_failed", &error.to_string());
            }
            eprintln!("worker preflight failed: {error}");
        }
        Err(panic) => {
            let message = if let Some(message) = panic.downcast_ref::<&str>() {
                (*message).to_owned()
            } else if let Some(message) = panic.downcast_ref::<String>() {
                message.clone()
            } else {
                "preflight thread panicked".to_owned()
            };
            if let Ok(db) = Database::open_connection(&state.database_path.to_string_lossy()) {
                let _ = db.fail_preflight(preflight_id, "validation_panic", &message);
            }
            eprintln!("worker preflight thread failed: {message}");
        }
    }
    match Database::open_connection(&state.database_path.to_string_lossy()) {
        Ok(db) => match db.get_preflight_view(preflight_id) {
            Ok(view) => {
                let event = match view.preflight.status.as_str() {
                    "completed" => "preflight.completed",
                    "cancelled" => "preflight.cancelled",
                    _ => "preflight.failed",
                };
                state.emit(event, &json!({"preflight":preflight_view_dto(&view)}));
            }
            Err(error) => {
                eprintln!("worker could not read terminal preflight {preflight_id}: {error}")
            }
        },
        Err(error) => {
            eprintln!("worker could not reopen terminal preflight {preflight_id}: {error}")
        }
    }
    state.finish_active_preflight(preflight_id);
}

fn run_scan_thread(
    state: Arc<SharedState>,
    engine: ScanEngine,
    run_id: i64,
    cancel_token: Arc<AtomicBool>,
) {
    let reporter = WorkerProgressReporter::new(state.clone(), run_id, cancel_token.clone());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.execute_started_run(run_id, &reporter)
    }));
    reporter.finish_progress();
    match outcome {
        Ok(Ok(_)) | Ok(Err(super_duper_core::Error::Cancelled)) => {}
        Ok(Err(error)) => {
            if let Ok(db) = Database::open_connection(&state.database_path.to_string_lossy()) {
                if matches!(db.get_scan_run(run_id), Ok(run) if run.status == "running" || run.status == "cancelling")
                {
                    if cancel_token.load(Ordering::Acquire) {
                        let _ = db.mark_run_cancelling(run_id);
                        let _ = db.cancel_scan_run(run_id);
                    } else {
                        let _ = db.fail_scan_run(run_id, &error.to_string());
                    }
                }
            }
            eprintln!("worker scan failed: {error}");
        }
        Err(panic) => {
            let message = if let Some(message) = panic.downcast_ref::<&str>() {
                (*message).to_owned()
            } else if let Some(message) = panic.downcast_ref::<String>() {
                message.clone()
            } else {
                "scan thread panicked".to_owned()
            };
            if let Ok(db) = Database::open_connection(&state.database_path.to_string_lossy()) {
                if cancel_token.load(Ordering::Acquire) {
                    let _ = db.mark_run_cancelling(run_id);
                    let _ = db.cancel_scan_run(run_id);
                } else {
                    let _ = db.fail_scan_run(run_id, &message);
                }
            }
            eprintln!("worker scan thread failed: {message}");
        }
    }

    match Database::open_connection(&state.database_path.to_string_lossy())
        .and_then(|db| db.get_scan_run(run_id))
    {
        Ok(run) => match run_dto(run) {
            Ok(run) => {
                let event = match run.status.as_str() {
                    "completed" => "run.completed",
                    "cancelled" => "run.cancelled",
                    _ => "run.failed",
                };
                state.emit(event, &json!({"run":run}));
            }
            Err(error) => eprintln!("worker could not encode terminal run: {}", error.message),
        },
        Err(error) => eprintln!("worker could not read terminal run {run_id}: {error}"),
    }
    state.finish_active(run_id);
}

fn log_scan_phase(run_id: i64, phase: &str, duration_secs: f64) {
    eprintln!(
        "performance kind=scan_phase run_id={run_id} phase={phase} duration_ms={:.3}",
        duration_secs * 1_000.0
    );
}

fn log_result_query(
    method: &str,
    run_id: i64,
    group_id: Option<i64>,
    page_size: i64,
    returned: usize,
    total: i64,
    duration: Duration,
) {
    let group = group_id.map_or_else(|| "-".to_owned(), |value| value.to_string());
    eprintln!(
        "performance kind=result_query method={method} run_id={run_id} group_id={group} \
         page_size={page_size} returned={returned} total={total} duration_ms={:.3}",
        duration.as_secs_f64() * 1_000.0
    );
}

struct WorkerProgressReporter {
    state: Arc<SharedState>,
    run_id: i64,
    cancel_token: Arc<AtomicBool>,
    progress: Mutex<ProgressState>,
    reducer: Mutex<ProgressReducer>,
    projection: Arc<(Mutex<LatestValueCoalescer<PendingProgress>>, Condvar)>,
    projection_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct ProgressState {
    phase: &'static str,
    files_discovered: usize,
    bytes_discovered: u64,
    files_hashed: usize,
    warning_count: usize,
    durable_warning_count: usize,
    phase_warning_base: usize,
    current_path: Option<String>,
    last_database_write: Option<Instant>,
}

impl WorkerProgressReporter {
    fn new(state: Arc<SharedState>, run_id: i64, cancel_token: Arc<AtomicBool>) -> Self {
        let projection_started = Instant::now();
        let projection = Arc::new((Mutex::new(LatestValueCoalescer::default()), Condvar::new()));
        let projection_thread = {
            let state = state.clone();
            let projection = projection.clone();
            let projection_cancel = cancel_token.clone();
            std::thread::Builder::new()
                .name(format!("scan-progress-{run_id}"))
                .spawn(move || {
                    progress_projection_loop(
                        state,
                        run_id,
                        projection_started,
                        projection_cancel,
                        projection,
                    )
                })
                .ok()
        };
        Self {
            state,
            run_id,
            cancel_token,
            progress: Mutex::new(ProgressState {
                phase: "discovering",
                files_discovered: 0,
                bytes_discovered: 0,
                files_hashed: 0,
                warning_count: 0,
                durable_warning_count: 0,
                phase_warning_base: 0,
                current_path: None,
                last_database_write: None,
            }),
            reducer: Mutex::new(ProgressReducer::new()),
            projection,
            projection_thread: Mutex::new(projection_thread),
        }
    }

    fn phase(&self, phase: &'static str) {
        if let Ok(db) = Database::open_connection(&self.state.database_path.to_string_lossy()) {
            if let Ok(run) = db.get_scan_run(self.run_id) {
                let mut progress = self
                    .progress
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                progress.files_discovered = run.files_discovered.max(0) as usize;
                progress.bytes_discovered = run.bytes_discovered.max(0) as u64;
                progress.files_hashed = run.files_hashed.max(0) as usize;
                progress.warning_count = run.warning_count.max(0) as usize;
                progress.durable_warning_count = progress.warning_count;
                progress.phase_warning_base = progress.warning_count;
            }
        }
        self.update(Some(phase), None, true);
    }

    fn update(&self, phase: Option<&'static str>, current_path: Option<&str>, force: bool) -> bool {
        let now = Instant::now();
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(phase) = phase {
            progress.phase = phase;
        }
        if let Some(path) = current_path.filter(|path| !path.is_empty()) {
            progress.current_path = Some(path.to_owned());
        }
        let write_database = force
            || progress.last_database_write.map_or(true, |last| {
                now.duration_since(last) >= DATABASE_PROGRESS_INTERVAL
            });
        if !write_database {
            return progress.warning_count <= progress.durable_warning_count;
        }

        let phase = progress.phase;
        let files_discovered = progress.files_discovered;
        let bytes_discovered = progress.bytes_discovered;
        let files_hashed = progress.files_hashed;
        let warning_count = progress.warning_count;
        progress.last_database_write = Some(now);
        drop(progress);

        match Database::open_connection(&self.state.database_path.to_string_lossy()) {
            Ok(db) => match db.update_run_progress_with_warning_accounting(
                self.run_id,
                phase,
                files_discovered as i64,
                bytes_discovered.min(i64::MAX as u64) as i64,
                files_hashed as i64,
                warning_count as i64,
            ) {
                Ok(()) => {
                    let mut progress = self
                        .progress
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    progress.durable_warning_count =
                        progress.durable_warning_count.max(warning_count);
                    true
                }
                Err(error) => {
                    if !matches!(db.get_scan_run(self.run_id), Ok(run) if run.status == "completed" || run.status == "cancelled" || run.status == "failed")
                    {
                        eprintln!(
                            "worker progress persistence failed for run {}: {error}",
                            self.run_id
                        );
                    }
                    false
                }
            },
            Err(error) => {
                if warning_count > 0 {
                    eprintln!(
                        "worker progress database unavailable for run {}: {error}",
                        self.run_id
                    );
                }
                false
            }
        }
    }

    fn finish_progress(&self) {
        let (projection, wake) = &*self.projection;
        projection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .terminate();
        wake.notify_all();
        if let Some(thread) = self
            .projection_thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = thread.join();
        }
    }
}

impl Drop for WorkerProgressReporter {
    fn drop(&mut self) {
        self.finish_progress();
    }
}

fn progress_projection_loop(
    state: Arc<SharedState>,
    run_id: i64,
    started: Instant,
    cancel_token: Arc<AtomicBool>,
    projection: Arc<(Mutex<LatestValueCoalescer<PendingProgress>>, Condvar)>,
) {
    let (coalescer, wake) = &*projection;
    loop {
        let emission = {
            let mut coalescer = coalescer
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            loop {
                if coalescer.is_terminal() {
                    return;
                }
                coalescer.latch_cancelling(cancel_token.load(Ordering::Acquire));
                let now_nanos = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
                if let Some(emission) = coalescer.take_due(now_nanos) {
                    break emission;
                }
                match coalescer.next_due_nanos() {
                    Some(due) => {
                        let wait_nanos = due.saturating_sub(now_nanos).max(1);
                        let (next, _) = wake
                            .wait_timeout(coalescer, Duration::from_nanos(wait_nanos))
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        coalescer = next;
                    }
                    None => {
                        coalescer = wake
                            .wait(coalescer)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }
                }
            }
        };
        emit_projected_progress(&state, run_id, &emission);
    }
}

fn emit_projected_progress(
    state: &SharedState,
    run_id: i64,
    emission: &progress_projection::Coalesced<PendingProgress>,
) {
    match progress_event_data(run_id, emission) {
        Ok(data) => state.emit("run.progress", &data),
        Err(error) => eprintln!("worker progress projection failed for run {run_id}: {error}"),
    }
}

impl ProgressReporter for WorkerProgressReporter {
    fn on_progress_observation(&self, observation: &ProgressObservation) {
        let snapshot = match self
            .reducer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .observe(observation.clone())
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                eprintln!(
                    "worker rejected scan progress observation for run {}: {error}",
                    self.run_id
                );
                return;
            }
        };
        let (legacy, warning_needs_persistence) = {
            let mut progress = self
                .progress
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            progress.phase = legacy_phase(snapshot.phase);
            progress.files_discovered = progress.files_discovered.max(
                snapshot
                    .counters
                    .discovered_files
                    .saturating_sub(snapshot.counters.zero_byte_files) as usize,
            );
            progress.bytes_discovered = progress
                .bytes_discovered
                .max(snapshot.counters.discovered_bytes);
            progress.files_hashed = progress
                .files_hashed
                .max(snapshot.counters.partial_hashes_succeeded as usize);
            progress.warning_count = progress.warning_count.max(snapshot.warning_count as usize);
            (
                LegacyProgressProjection {
                    phase: progress.phase,
                    files_discovered: progress.files_discovered,
                    bytes_discovered: progress.bytes_discovered,
                    files_hashed: progress.files_hashed,
                    warning_count: progress.warning_count,
                    current_path: progress.current_path.clone(),
                },
                progress.warning_count > progress.durable_warning_count,
            )
        };
        if warning_needs_persistence && !self.update(None, None, true) {
            return;
        }
        {
            let (projection, wake) = &*self.projection;
            projection
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .submit(
                    PendingProgress { snapshot, legacy },
                    self.cancel_token.load(Ordering::Acquire),
                );
            wake.notify_one();
        }
    }

    fn on_scan_start(&self) {
        self.phase("discovering");
    }

    fn on_discovery_progress(
        &self,
        files_found: usize,
        bytes_found: u64,
        warning_count: usize,
        current_path: &str,
    ) {
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        progress.files_discovered = progress.files_discovered.max(files_found);
        progress.bytes_discovered = progress.bytes_discovered.max(bytes_found);
        progress.warning_count = progress.warning_count.max(warning_count);
        drop(progress);
        self.update(None, Some(current_path), false);
    }

    fn on_scan_complete(&self, _total_files: usize, duration_secs: f64) {
        log_scan_phase(self.run_id, "discovering", duration_secs);
    }

    fn on_hash_start(&self) {
        self.phase("hashing");
    }

    fn on_hash_progress_detailed(
        &self,
        files_hashed: usize,
        _total_files: usize,
        warning_count: usize,
        current_path: Option<&str>,
    ) {
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        progress.files_hashed = progress.files_hashed.max(files_hashed);
        progress.warning_count = progress.phase_warning_base + warning_count;
        drop(progress);
        self.update(None, current_path, false);
    }

    fn on_hash_complete(&self, _total_dupes: usize, duration_secs: f64) {
        log_scan_phase(self.run_id, "hashing", duration_secs);
    }

    fn on_db_write_start(&self) {
        self.phase("persisting");
    }

    fn on_db_write_progress(&self, _rows: usize, _total_rows: usize) {
        self.update(None, None, false);
    }

    fn on_db_write_complete(&self, _rows: usize, duration_secs: f64) {
        log_scan_phase(self.run_id, "persisting", duration_secs);
    }

    fn on_dir_analysis_start(&self) {
        self.phase("analyzing_folders");
    }

    fn on_dir_analysis_progress(&self, _completed: usize, _total: usize) {
        self.update(None, None, false);
    }

    fn on_dir_analysis_complete(
        &self,
        _fingerprints: usize,
        _similarity_pairs: usize,
        duration_secs: f64,
    ) {
        log_scan_phase(self.run_id, "analyzing_folders", duration_secs);
    }

    fn on_finalizing(&self) {
        self.phase("finalizing");
    }

    fn on_finalizing_complete(&self, duration_secs: f64) {
        log_scan_phase(self.run_id, "finalizing", duration_secs);
    }
}

fn legacy_phase(phase: TelemetryPhase) -> &'static str {
    match phase {
        TelemetryPhase::Discovering => "discovering",
        TelemetryPhase::CandidateScreening | TelemetryPhase::FullHashing => "hashing",
        TelemetryPhase::Persisting => "persisting",
        TelemetryPhase::AnalyzingFolders => "analyzing_folders",
        TelemetryPhase::Finalizing => "finalizing",
        TelemetryPhase::Overall => "discovering",
    }
}

pub fn run<R: BufRead, W: Write + Send>(input: R, output: W) -> Result<(), WorkerError> {
    run_with_options(input, output, WorkerOptions::default())
}

pub fn run_with_options<R: BufRead, W: Write + Send>(
    mut input: R,
    mut output: W,
    options: WorkerOptions,
) -> Result<(), WorkerError> {
    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel::<String>();
        let writer = scope.spawn(move || write_output(receiver, &mut output));
        let state = SharedState::new(options, sender.clone())?;
        let mut session = WorkerSession::new(state.clone());
        let dispatch_result = (|| loop {
            let Some(line) = read_frame(&mut input)? else {
                return Ok(());
            };
            let response = session.handle_line(&line)?;
            sender.send(response).map_err(|_| {
                WorkerError::Io(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "worker protocol output closed",
                ))
            })?;
        })();

        state.shutdown();
        drop(session);
        drop(state);
        drop(sender);
        let writer_result = writer.join().map_err(|_| {
            WorkerError::Io(io::Error::new(
                io::ErrorKind::Other,
                "worker output thread panicked",
            ))
        })?;
        dispatch_result.and(writer_result)
    })
}

fn write_output<W: Write>(receiver: Receiver<String>, output: &mut W) -> Result<(), WorkerError> {
    for frame in receiver {
        output.write_all(frame.as_bytes())?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

fn read_frame<R: BufRead>(input: &mut R) -> Result<Option<String>, WorkerError> {
    let mut frame = Vec::new();
    let mut limited_input = std::io::Read::take(input, (MAXIMUM_FRAME_BYTES + 2) as u64);
    let bytes_read = limited_input.read_until(b'\n', &mut frame)?;
    if bytes_read == 0 {
        return Ok(None);
    }
    let terminated = frame.last() == Some(&b'\n');
    if terminated {
        frame.pop();
        if frame.last() == Some(&b'\r') {
            frame.pop();
        }
    }
    if frame.len() > MAXIMUM_FRAME_BYTES {
        return Err(WorkerError::FatalProtocol(format!(
            "input frame exceeds {MAXIMUM_FRAME_BYTES} bytes"
        )));
    }
    if !terminated {
        return Err(WorkerError::FatalProtocol(
            "input ended with a partial protocol frame".to_owned(),
        ));
    }
    let line = std::str::from_utf8(&frame).map_err(|error| {
        WorkerError::FatalProtocol(format!("input frame is not valid UTF-8: {error}"))
    })?;
    Ok(Some(line.to_owned()))
}

fn parse_parameters<T: for<'de> Deserialize<'de>>(
    request: &RequestEnvelope,
) -> Result<T, ProtocolFailure> {
    serde_json::from_value(request.params.clone()).map_err(|error| {
        ProtocolFailure::new(
            "invalid_request",
            format!("Invalid {} parameters: {error}", request.method),
        )
    })
}

fn validate_page(page: &PageParameters) -> Result<(), ProtocolFailure> {
    validate_page_values(page.offset, page.limit)
}

fn validate_page_values(offset: i64, limit: i64) -> Result<(), ProtocolFailure> {
    if offset < 0 || !(1..=MAXIMUM_PAGE_SIZE).contains(&limit) {
        Err(ProtocolFailure::new(
            "invalid_request",
            format!("offset must be non-negative and limit must be 1..={MAXIMUM_PAGE_SIZE}"),
        ))
    } else {
        Ok(())
    }
}

fn validate_result_page_size(page_size: i64) -> Result<(), ProtocolFailure> {
    if (1..=MAXIMUM_PAGE_SIZE).contains(&page_size) {
        Ok(())
    } else {
        Err(ProtocolFailure::new(
            "invalid_request",
            format!("pageSize must be 1..={MAXIMUM_PAGE_SIZE}"),
        ))
    }
}

fn validate_preference_rule_parameters(
    parameters: &PreferenceRuleSaveParameters,
) -> Result<(), ProtocolFailure> {
    if parameters.operation_id.is_empty()
        || parameters.operation_id.chars().count() > MAXIMUM_OPERATION_ID_CHARACTERS
    {
        return Err(ProtocolFailure::new(
            "invalid_request",
            format!("operationId must contain 1 to {MAXIMUM_OPERATION_ID_CHARACTERS} characters"),
        )
        .with_details(json!({"field":"operationId"})));
    }
    if parameters.name != parameters.name.trim()
        || parameters.name.trim().is_empty()
        || parameters.name.chars().count() > 128
    {
        return Err(ProtocolFailure::new(
            "invalid_request",
            "name must contain 1 to 128 characters without surrounding whitespace",
        )
        .with_details(json!({"field":"name"})));
    }
    if !(1..=64).contains(&parameters.roots.len()) {
        return Err(ProtocolFailure::new(
            "invalid_request",
            "roots must contain 1 to 64 ordered paths",
        )
        .with_details(json!({"field":"roots","minimum":1,"maximum":64})));
    }
    if parameters.expected_revision < 0
        || (parameters.rule_id.is_none() && parameters.expected_revision != 0)
    {
        return Err(ProtocolFailure::new(
            "invalid_request",
            "new rules require expectedRevision zero; revisions cannot be negative",
        )
        .with_details(json!({"field":"expectedRevision"})));
    }
    let mut distinct = std::collections::HashSet::new();
    for root in &parameters.roots {
        if root != root.trim()
            || root.is_empty()
            || root.chars().count() > MAXIMUM_EXACT_PATH_CHARACTERS
            || !Path::new(root).is_absolute()
        {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "each root must be a nonblank absolute path of at most 32767 characters",
            )
            .with_details(json!({"field":"roots"})));
        }
        if !distinct.insert(root.to_lowercase()) {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "roots must be unique ignoring case",
            )
            .with_details(json!({"field":"roots"})));
        }
    }
    Ok(())
}

fn validate_operation_id(operation_id: &str) -> Result<(), ProtocolFailure> {
    if operation_id.is_empty() || operation_id.chars().count() > MAXIMUM_OPERATION_ID_CHARACTERS {
        return Err(ProtocolFailure::new(
            "invalid_request",
            format!("operationId must contain 1 to {MAXIMUM_OPERATION_ID_CHARACTERS} characters"),
        )
        .with_details(json!({"field":"operationId"})));
    }
    Ok(())
}

fn parse_preference_preview_scope(
    scope: &mut PreferencePreviewScopeParameters,
) -> Result<(PreferencePreviewScope, Value), ProtocolFailure> {
    match scope.kind.as_str() {
        "completed_run" => {
            if !scope.group_ids.is_empty()
                || scope.filter != DuplicateFileGroupFilterParameters::default()
            {
                return Err(ProtocolFailure::new(
                    "invalid_scope",
                    "completed_run scope does not accept groupIds or filter fields",
                ));
            }
            Ok((
                PreferencePreviewScope::CompletedRun,
                json!({"kind":"completed_run"}),
            ))
        }
        "selected_sets" => {
            if !(1..=500).contains(&scope.group_ids.len()) {
                return Err(ProtocolFailure::new(
                    "invalid_scope",
                    "selected_sets scope requires 1 to 500 groupIds",
                )
                .with_details(json!({"field":"scope.groupIds","minimum":1,"maximum":500})));
            }
            if scope.filter != DuplicateFileGroupFilterParameters::default() {
                return Err(ProtocolFailure::new(
                    "invalid_scope",
                    "selected_sets scope does not accept filter fields",
                ));
            }
            if scope.group_ids.iter().any(|id| *id <= 0) {
                return Err(ProtocolFailure::new(
                    "invalid_scope",
                    "scope.groupIds must contain positive IDs",
                ));
            }
            let mut ids = scope.group_ids.clone();
            ids.sort_unstable();
            ids.dedup();
            if ids.len() != scope.group_ids.len() {
                return Err(ProtocolFailure::new(
                    "invalid_scope",
                    "scope.groupIds must not contain duplicates",
                ));
            }
            Ok((
                PreferencePreviewScope::SelectedSets(ids.clone()),
                json!({"kind":"selected_sets","groupIds":ids}),
            ))
        }
        "current_filter" => {
            if !scope.group_ids.is_empty() {
                return Err(ProtocolFailure::new(
                    "invalid_scope",
                    "current_filter scope does not accept groupIds",
                ));
            }
            let requested_path_match = parse_path_match(&scope.filter.path_match)?;
            let search =
                validate_group_path_filter(scope.filter.search.take(), requested_path_match)?;
            let path_match = if search.is_none() {
                DuplicateFilePathMatchMode::Substring
            } else {
                requested_path_match
            };
            let extension = normalize_extension_filter(scope.filter.extension.as_deref())?;
            let extension_match =
                normalize_extension_match(&scope.filter.extension_match, extension.as_deref())?;
            let minimum_size =
                parse_non_negative_decimal(&scope.filter.minimum_size, "scope.filter.minimumSize")?;
            validate_minimum_copy_count(scope.filter.minimum_copy_count)?;
            let selected_root = scope
                .filter
                .selected_root
                .clone()
                .filter(|value| !value.is_empty());
            let selected_drive = scope
                .filter
                .selected_drive
                .clone()
                .filter(|value| !value.is_empty());
            let signature = json!({
                "kind":"current_filter",
                "search":search.as_deref().unwrap_or_default(),
                "pathMatch":path_match_name(path_match),
                "extension":extension,
                "extensionMatch":extension_match_name(extension_match),
                "minimumSize":minimum_size,
                "minimumCopyCount":scope.filter.minimum_copy_count,
                "acrossDrives":scope.filter.across_drives,
                "selectedRoot":selected_root.as_deref().unwrap_or_default(),
                "selectedDrive":selected_drive.as_deref().unwrap_or_default(),
            });
            Ok((
                PreferencePreviewScope::CurrentFilter(DuplicateFileGroupFilter {
                    search,
                    path_match,
                    extension_key: extension,
                    extension_match,
                    minimum_size,
                    minimum_copy_count: scope.filter.minimum_copy_count,
                    across_drives: scope.filter.across_drives,
                    selected_root,
                    selected_drive,
                }),
                signature,
            ))
        }
        _ => Err(ProtocolFailure::new(
            "invalid_scope",
            "scope.kind must be selected_sets, current_filter, or completed_run",
        )
        .with_details(json!({
            "field":"scope.kind",
            "allowed":["selected_sets","current_filter","completed_run"]
        }))),
    }
}

fn parse_path_match(value: &str) -> Result<DuplicateFilePathMatchMode, ProtocolFailure> {
    match value {
        "substring" => Ok(DuplicateFilePathMatchMode::Substring),
        "exact" => Ok(DuplicateFilePathMatchMode::Exact),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "filter.pathMatch must be substring or exact",
        )
        .with_details(json!({
            "field":"filter.pathMatch",
            "allowed":["substring","exact"]
        }))),
    }
}

fn parse_group_sort_field(value: &str) -> Result<DuplicateFileGroupSortField, ProtocolFailure> {
    match value {
        "recoverableBytes" => Ok(DuplicateFileGroupSortField::RecoverableBytes),
        "groupSize" => Ok(DuplicateFileGroupSortField::GroupSize),
        "copyCount" => Ok(DuplicateFileGroupSortField::CopyCount),
        "representativeName" => Ok(DuplicateFileGroupSortField::RepresentativeName),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.field is not allowed for duplicate file groups",
        )
        .with_details(json!({
            "field":"sort.field",
            "allowed":["recoverableBytes","groupSize","copyCount","representativeName"]
        }))),
    }
}

fn parse_warning_sort_field(value: &str) -> Result<RunWarningSortField, ProtocolFailure> {
    match value {
        "phase" => Ok(RunWarningSortField::Phase),
        "occurrenceCount" => Ok(RunWarningSortField::OccurrenceCount),
        "message" => Ok(RunWarningSortField::Message),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.field is not allowed for run warnings",
        )
        .with_details(json!({
            "field":"sort.field",
            "allowed":["phase","occurrenceCount","message"]
        }))),
    }
}

fn parse_selected_root_facet_sort_field(
    value: &str,
) -> Result<DuplicateFileSelectedRootFacetSortField, ProtocolFailure> {
    match value {
        "matchingGroupCount" => Ok(DuplicateFileSelectedRootFacetSortField::MatchingGroupCount),
        "value" => Ok(DuplicateFileSelectedRootFacetSortField::Value),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.field is not allowed for duplicate file selected-root facets",
        )
        .with_details(json!({
            "field":"sort.field",
            "allowed":["matchingGroupCount","value"]
        }))),
    }
}

fn parse_drive_facet_sort_field(
    value: &str,
) -> Result<DuplicateFileDriveFacetSortField, ProtocolFailure> {
    match value {
        "matchingGroupCount" => Ok(DuplicateFileDriveFacetSortField::MatchingGroupCount),
        "value" => Ok(DuplicateFileDriveFacetSortField::Value),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.field is not allowed for duplicate file drive facets",
        )
        .with_details(json!({
            "field":"sort.field",
            "allowed":["matchingGroupCount","value"]
        }))),
    }
}

fn parse_member_sort_field(value: &str) -> Result<DuplicateFileMemberSortField, ProtocolFailure> {
    match value {
        "path" => Ok(DuplicateFileMemberSortField::Path),
        "modifiedTime" => Ok(DuplicateFileMemberSortField::ModifiedTime),
        "size" => Ok(DuplicateFileMemberSortField::Size),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.field is not allowed for duplicate file members",
        )
        .with_details(json!({
            "field":"sort.field",
            "allowed":["path","modifiedTime","size"]
        }))),
    }
}

fn parse_folder_group_sort_field(
    value: &str,
) -> Result<DuplicateFolderGroupSortField, ProtocolFailure> {
    match value {
        "totalBytes" => Ok(DuplicateFolderGroupSortField::TotalBytes),
        "copyCount" => Ok(DuplicateFolderGroupSortField::CopyCount),
        "fileCount" => Ok(DuplicateFolderGroupSortField::FileCount),
        "representativePath" => Ok(DuplicateFolderGroupSortField::RepresentativePath),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.field is not allowed for duplicate folder groups",
        )
        .with_details(json!({
            "field":"sort.field",
            "allowed":["totalBytes","copyCount","fileCount","representativePath"]
        }))),
    }
}

fn parse_folder_member_sort_field(
    value: &str,
) -> Result<DuplicateFolderMemberSortField, ProtocolFailure> {
    match value {
        "path" => Ok(DuplicateFolderMemberSortField::Path),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.field is not allowed for duplicate folder members",
        )
        .with_details(json!({"field":"sort.field","allowed":["path"]}))),
    }
}

fn parse_sort_direction(value: &str) -> Result<SortDirection, ProtocolFailure> {
    match value {
        "ascending" => Ok(SortDirection::Ascending),
        "descending" => Ok(SortDirection::Descending),
        _ => Err(ProtocolFailure::new(
            "invalid_request",
            "sort.direction must be ascending or descending",
        )
        .with_details(json!({"field":"sort.direction"}))),
    }
}

fn validate_search(value: Option<String>) -> Result<Option<String>, ProtocolFailure> {
    let value = value
        .map(|search| search.trim().to_owned())
        .filter(|search| !search.is_empty());
    if value
        .as_ref()
        .is_some_and(|search| search.chars().count() > MAXIMUM_FILTER_CHARACTERS)
    {
        Err(ProtocolFailure::new(
            "invalid_request",
            format!("filter.search may contain at most {MAXIMUM_FILTER_CHARACTERS} characters"),
        )
        .with_details(json!({"field":"filter.search"})))
    } else {
        Ok(value)
    }
}

fn validate_group_path_filter(
    value: Option<String>,
    path_match: DuplicateFilePathMatchMode,
) -> Result<Option<String>, ProtocolFailure> {
    if path_match == DuplicateFilePathMatchMode::Substring {
        return validate_search(value);
    }
    let value = value.filter(|search| !search.trim().is_empty());
    if value
        .as_ref()
        .is_some_and(|search| search.chars().count() > MAXIMUM_EXACT_PATH_CHARACTERS)
    {
        Err(ProtocolFailure::new(
            "invalid_request",
            format!(
                "filter.search may contain at most {MAXIMUM_EXACT_PATH_CHARACTERS} characters for exact path matching"
            ),
        )
        .with_details(json!({"field":"filter.search"})))
    } else {
        Ok(value)
    }
}

fn normalize_extension_filter(value: Option<&str>) -> Result<Option<String>, ProtocolFailure> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.chars().count() > MAXIMUM_EXTENSION_CHARACTERS
        || value.contains('.')
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(ProtocolFailure::new(
            "invalid_request",
            format!(
                "filter.extension must be empty for no extension or contain at most {MAXIMUM_EXTENSION_CHARACTERS} characters without a dot or path separator"
            ),
        )
        .with_details(json!({"field":"filter.extension"})));
    }
    Ok(Some(value.to_lowercase()))
}

fn normalize_extension_match(
    value: &str,
    extension: Option<&str>,
) -> Result<DuplicateFileExtensionMatchMode, ProtocolFailure> {
    let parsed = match value {
        "any" => DuplicateFileExtensionMatchMode::AnyMember,
        "all" => DuplicateFileExtensionMatchMode::AllMembers,
        _ => {
            return Err(ProtocolFailure::new(
                "invalid_request",
                "filter.extensionMatch must be any or all",
            )
            .with_details(json!({
                "field":"filter.extensionMatch",
                "allowed":["any","all"]
            })))
        }
    };
    Ok(extension.map_or(DuplicateFileExtensionMatchMode::AnyMember, |_| parsed))
}

fn parse_non_negative_decimal(value: &str, field: &str) -> Result<i64, ProtocolFailure> {
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .ok_or_else(|| {
            ProtocolFailure::new(
                "invalid_request",
                format!("{field} must be a non-negative decimal string"),
            )
            .with_details(json!({"field":field}))
        })
}

fn validate_minimum_copy_count(value: i64) -> Result<(), ProtocolFailure> {
    if value >= 2 {
        Ok(())
    } else {
        Err(ProtocolFailure::new(
            "invalid_request",
            "filter.minimumCopyCount must be an integer greater than or equal to 2",
        )
        .with_details(json!({"field":"filter.minimumCopyCount"})))
    }
}

fn group_query_signature(
    run_id: i64,
    sort_field: DuplicateFileGroupSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
    path_match: DuplicateFilePathMatchMode,
    extension: Option<&str>,
    extension_match: DuplicateFileExtensionMatchMode,
    minimum_size: i64,
    minimum_copy_count: i64,
    across_drives: bool,
    selected_root: Option<&str>,
    selected_drive: Option<&str>,
) -> String {
    json!({
        "runId": run_id,
        "sortField": group_sort_name(sort_field),
        "sortDirection": direction_name(sort_direction),
        "search": search.unwrap_or_default(),
        "pathMatch": path_match_name(path_match),
        "extension": extension,
        "extensionMatch": extension_match_name(extension_match),
        "minimumSize": minimum_size,
        "minimumCopyCount": minimum_copy_count,
        "acrossDrives": across_drives,
        "selectedRoot": selected_root.unwrap_or_default(),
        "selectedDrive": selected_drive.unwrap_or_default(),
    })
    .to_string()
}

fn selected_root_facet_query_signature(
    run_id: i64,
    sort_field: DuplicateFileSelectedRootFacetSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
    path_match: DuplicateFilePathMatchMode,
    extension: Option<&str>,
    extension_match: DuplicateFileExtensionMatchMode,
    minimum_size: i64,
    minimum_copy_count: i64,
    across_drives: bool,
    selected_drive: Option<&str>,
) -> String {
    json!({
        "runId": run_id,
        "sortField": selected_root_facet_sort_name(sort_field),
        "sortDirection": direction_name(sort_direction),
        "search": search.unwrap_or_default(),
        "pathMatch": path_match_name(path_match),
        "extension": extension,
        "extensionMatch": extension_match_name(extension_match),
        "minimumSize": minimum_size,
        "minimumCopyCount": minimum_copy_count,
        "acrossDrives": across_drives,
        "selectedDrive": selected_drive.unwrap_or_default(),
    })
    .to_string()
}

fn drive_facet_query_signature(
    run_id: i64,
    sort_field: DuplicateFileDriveFacetSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
    path_match: DuplicateFilePathMatchMode,
    extension: Option<&str>,
    extension_match: DuplicateFileExtensionMatchMode,
    minimum_size: i64,
    minimum_copy_count: i64,
    across_drives: bool,
    selected_root: Option<&str>,
) -> String {
    json!({
        "runId": run_id,
        "sortField": drive_facet_sort_name(sort_field),
        "sortDirection": direction_name(sort_direction),
        "search": search.unwrap_or_default(),
        "pathMatch": path_match_name(path_match),
        "extension": extension,
        "extensionMatch": extension_match_name(extension_match),
        "minimumSize": minimum_size,
        "minimumCopyCount": minimum_copy_count,
        "acrossDrives": across_drives,
        "selectedRoot": selected_root.unwrap_or_default(),
    })
    .to_string()
}

fn member_query_signature(
    run_id: i64,
    group_id: i64,
    sort_field: DuplicateFileMemberSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
) -> String {
    format!(
        "{run_id}|{group_id}|{}|{}|{}",
        member_sort_name(sort_field),
        direction_name(sort_direction),
        search.unwrap_or_default()
    )
}

fn folder_group_query_signature(
    run_id: i64,
    sort_field: DuplicateFolderGroupSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
    minimum_size: i64,
) -> String {
    format!(
        "{run_id}|{}|{}|{}|{minimum_size}",
        folder_group_sort_name(sort_field),
        direction_name(sort_direction),
        search.unwrap_or_default()
    )
}

fn folder_member_query_signature(
    run_id: i64,
    group_id: i64,
    sort_field: DuplicateFolderMemberSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
    review_plan_id: i64,
    review_revision: i64,
) -> String {
    format!(
        "{run_id}|{group_id}|{}|{}|{}|{review_plan_id}|{review_revision}",
        folder_member_sort_name(sort_field),
        direction_name(sort_direction),
        search.unwrap_or_default()
    )
}

fn encode_group_cursor(
    group: &DuplicateFileGroupResult,
    sort_field: DuplicateFileGroupSortField,
    before: bool,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    let value = match sort_field {
        DuplicateFileGroupSortField::RecoverableBytes => {
            CursorScalar::Integer(group.recoverable_bytes.to_string())
        }
        DuplicateFileGroupSortField::GroupSize => {
            CursorScalar::Integer(group.file_size.to_string())
        }
        DuplicateFileGroupSortField::CopyCount => {
            CursorScalar::Integer(group.file_count.to_string())
        }
        DuplicateFileGroupSortField::RepresentativeName => {
            CursorScalar::Text(group.representative_name.clone())
        }
    };
    encode_cursor(CursorPayload {
        version: 1,
        kind: "duplicate-file-groups".to_owned(),
        query: signature.to_owned(),
        before,
        value,
        id: group.id,
    })
}

fn encode_warning_cursor(
    warning: &RunWarningAggregate,
    sort_field: RunWarningSortField,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    let value = match sort_field {
        RunWarningSortField::Phase => CursorScalar::Text(warning.phase.clone()),
        RunWarningSortField::OccurrenceCount => {
            CursorScalar::Integer(warning.occurrence_count.to_string())
        }
        RunWarningSortField::Message => CursorScalar::Text(warning.message.clone()),
    };
    encode_cursor(CursorPayload {
        version: 1,
        kind: "run-warnings".to_owned(),
        query: signature.to_owned(),
        before: false,
        value,
        id: warning.id,
    })
}

fn encode_selected_root_facet_cursor(
    facet: &DuplicateFileSelectedRootFacetResult,
    sort_field: DuplicateFileSelectedRootFacetSortField,
    before: bool,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    let value = match sort_field {
        DuplicateFileSelectedRootFacetSortField::MatchingGroupCount => {
            CursorScalar::Integer(facet.matching_group_count.to_string())
        }
        DuplicateFileSelectedRootFacetSortField::Value => CursorScalar::Text(facet.value.clone()),
    };
    encode_cursor(CursorPayload {
        version: 1,
        kind: "duplicate-file-selected-root-facets".to_owned(),
        query: signature.to_owned(),
        before,
        value,
        id: facet.cursor_id,
    })
}

fn encode_drive_facet_cursor(
    facet: &DuplicateFileDriveFacetResult,
    sort_field: DuplicateFileDriveFacetSortField,
    before: bool,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    let value = match sort_field {
        DuplicateFileDriveFacetSortField::MatchingGroupCount => {
            CursorScalar::Integer(facet.matching_group_count.to_string())
        }
        DuplicateFileDriveFacetSortField::Value => CursorScalar::Text(facet.value.clone()),
    };
    encode_cursor(CursorPayload {
        version: 1,
        kind: "duplicate-file-drive-facets".to_owned(),
        query: signature.to_owned(),
        before,
        value,
        id: facet.cursor_id,
    })
}

fn encode_member_cursor(
    member: &DuplicateFileMemberResult,
    sort_field: DuplicateFileMemberSortField,
    before: bool,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    let value = match sort_field {
        DuplicateFileMemberSortField::Path => CursorScalar::Text(member.canonical_path.clone()),
        DuplicateFileMemberSortField::ModifiedTime => {
            CursorScalar::Integer(member.last_modified.to_string())
        }
        DuplicateFileMemberSortField::Size => CursorScalar::Integer(member.file_size.to_string()),
    };
    encode_cursor(CursorPayload {
        version: 1,
        kind: "duplicate-file-members".to_owned(),
        query: signature.to_owned(),
        before,
        value,
        id: member.id,
    })
}

fn encode_review_group_cursor(group_id: i64, signature: &str) -> Result<String, ProtocolFailure> {
    encode_cursor(CursorPayload {
        version: 1,
        kind: "review-groups".to_owned(),
        query: signature.to_owned(),
        before: false,
        value: CursorScalar::Integer(group_id.to_string()),
        id: group_id,
    })
}

fn encode_preference_preview_cursor(
    group_id: i64,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    encode_cursor(CursorPayload {
        version: 1,
        kind: "preference-rule-preview".to_owned(),
        query: signature.to_owned(),
        before: false,
        value: CursorScalar::Integer(group_id.to_string()),
        id: group_id,
    })
}

fn encode_preference_application_cursor(
    application_id: i64,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    encode_cursor(CursorPayload {
        version: 1,
        kind: "preference-rule-applications".to_owned(),
        query: signature.to_owned(),
        before: false,
        value: CursorScalar::Integer(application_id.to_string()),
        id: application_id,
    })
}

fn encode_review_folder_group_cursor(
    folder_group_id: i64,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    encode_cursor(CursorPayload {
        version: 1,
        kind: "review-folder-groups".to_owned(),
        query: signature.to_owned(),
        before: false,
        value: CursorScalar::Integer(folder_group_id.to_string()),
        id: folder_group_id,
    })
}

fn encode_folder_group_cursor(
    group: &DuplicateFolderGroupResult,
    sort_field: DuplicateFolderGroupSortField,
    before: bool,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    let value = match sort_field {
        DuplicateFolderGroupSortField::TotalBytes => {
            CursorScalar::Integer(group.total_size.to_string())
        }
        DuplicateFolderGroupSortField::CopyCount => {
            CursorScalar::Integer(group.folder_count.to_string())
        }
        DuplicateFolderGroupSortField::FileCount => {
            CursorScalar::Integer(group.file_count.to_string())
        }
        DuplicateFolderGroupSortField::RepresentativePath => {
            CursorScalar::Text(group.representative_path.clone())
        }
    };
    encode_cursor(CursorPayload {
        version: 1,
        kind: "duplicate-folder-groups".to_owned(),
        query: signature.to_owned(),
        before,
        value,
        id: group.id,
    })
}

fn encode_folder_member_cursor(
    member: &DuplicateFolderMemberResult,
    before: bool,
    signature: &str,
) -> Result<String, ProtocolFailure> {
    encode_cursor(CursorPayload {
        version: 1,
        kind: "duplicate-folder-members".to_owned(),
        query: signature.to_owned(),
        before,
        value: CursorScalar::Text(member.path.clone()),
        id: member.id,
    })
}

fn encode_cursor(payload: CursorPayload) -> Result<String, ProtocolFailure> {
    let bytes = serde_json::to_vec(&payload).map_err(|error| {
        ProtocolFailure::new(
            "internal_error",
            format!("Could not encode page cursor: {error}"),
        )
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn decode_cursor(
    encoded: Option<&str>,
    expected_kind: &str,
    expected_query: &str,
) -> Result<Option<PageCursor>, ProtocolFailure> {
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    if encoded.is_empty() || encoded.len() > MAXIMUM_CURSOR_CHARACTERS || encoded.len() % 2 != 0 {
        return Err(invalid_cursor());
    }
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).map_err(|_| invalid_cursor())?;
            u8::from_str_radix(digits, 16).map_err(|_| invalid_cursor())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let payload: CursorPayload = serde_json::from_slice(&bytes).map_err(|_| invalid_cursor())?;
    if payload.version != 1
        || payload.kind != expected_kind
        || payload.query != expected_query
        || payload.id <= 0
    {
        return Err(invalid_cursor());
    }
    let value = match payload.value {
        CursorScalar::Integer(value) => {
            PageCursorValue::Integer(value.parse::<i64>().map_err(|_| invalid_cursor())?)
        }
        CursorScalar::Text(value) => PageCursorValue::Text(value),
    };
    Ok(Some(PageCursor {
        value,
        id: payload.id,
        before: payload.before,
    }))
}

fn invalid_cursor() -> ProtocolFailure {
    ProtocolFailure::new(
        "invalid_cursor",
        "The page cursor is invalid or belongs to a different query",
    )
}

fn validate_cursor_value(
    cursor: Option<&PageCursor>,
    expects_text: bool,
) -> Result<(), ProtocolFailure> {
    match (cursor.map(|cursor| &cursor.value), expects_text) {
        (None, _)
        | (Some(PageCursorValue::Text(_)), true)
        | (Some(PageCursorValue::Integer(_)), false) => Ok(()),
        _ => Err(invalid_cursor()),
    }
}

fn group_dto(group: DuplicateFileGroupResult) -> DuplicateFileGroupDto {
    let representative_type = Path::new(&group.representative_name)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| format!(".{}", value.to_ascii_lowercase()))
        .unwrap_or_else(|| "File".to_owned());
    DuplicateFileGroupDto {
        id: group.id,
        run_id: group.run_id,
        group_size: group.file_size.to_string(),
        copy_count: group.file_count,
        recoverable_bytes: group.recoverable_bytes.to_string(),
        representative_name: group.representative_name,
        representative_type,
        distinct_selected_root_count: group.distinct_selected_root_count,
        distinct_drive_count: group.distinct_drive_count,
    }
}

fn review_summary_dto(
    summary: &super_duper_core::storage::models::DuplicateFileReviewSummary,
) -> DuplicateFileReviewSummaryDto {
    DuplicateFileReviewSummaryDto {
        matching_group_count: summary.matching_group_count,
        matching_copy_count: summary.matching_copy_count,
        potential_recoverable_bytes: summary.potential_recoverable_bytes.to_string(),
        largest_recoverable_bytes: summary.largest_recoverable_bytes.to_string(),
        distinct_selected_root_count: summary.distinct_selected_root_count,
        distinct_drive_count: summary.distinct_drive_count,
        across_drive_group_count: summary.across_drive_group_count,
    }
}

fn selected_root_facet_dto(
    facet: DuplicateFileSelectedRootFacetResult,
) -> DuplicateFileSelectedRootFacetDto {
    DuplicateFileSelectedRootFacetDto {
        value: facet.value,
        matching_group_count: facet.matching_group_count,
    }
}

fn drive_facet_dto(facet: DuplicateFileDriveFacetResult) -> DuplicateFileDriveFacetDto {
    DuplicateFileDriveFacetDto {
        value: facet.value,
        matching_group_count: facet.matching_group_count,
    }
}

fn member_dto(member: DuplicateFileMemberResult) -> DuplicateFileMemberDto {
    DuplicateFileMemberDto {
        id: member.id,
        group_id: member.group_id,
        path: member.canonical_path,
        file_name: member.file_name,
        parent_path: member.parent_dir,
        root_path: member.root_path,
        relative_path: member.relative_path,
        drive_letter: member.drive_letter,
        size: member.file_size.to_string(),
        modified_time_unix_nanos: member.last_modified.to_string(),
        decision: member.review_decision.as_str().to_owned(),
        decision_provenance: member.review_provenance,
        decision_at: member.review_decided_at,
        decision_application_id: member.review_application_id,
        validation_state: member.validation_state,
        validation_reason_code: member.validation_reason_code,
        validation_observed_at: member.validation_observed_at,
        invalidated_decision: member
            .invalidated_decision
            .map(|value| value.as_str().to_owned()),
    }
}

fn review_plan_dto(run_id: i64, view: &ReviewPlanView) -> ReviewPlanDto {
    match &view.plan {
        Some(plan) => ReviewPlanDto {
            id: Some(plan.id),
            run_id: plan.run_id,
            state: plan.state.clone(),
            revision: plan.revision,
            created_at: Some(plan.created_at.clone()),
            updated_at: Some(plan.updated_at.clone()),
        },
        None => ReviewPlanDto {
            id: None,
            run_id,
            state: "notCreated".to_owned(),
            revision: 0,
            created_at: None,
            updated_at: None,
        },
    }
}

fn review_plan_summary_dto(summary: &ReviewPlanSummary) -> ReviewPlanSummaryDto {
    ReviewPlanSummaryDto {
        decided_group_count: summary.decided_group_count,
        keep_count: summary.keep_count,
        remove_count: summary.remove_count,
        undecided_count: summary.undecided_count,
        decided_folder_group_count: summary.decided_folder_group_count,
        folder_keep_count: summary.folder_keep_count,
        folder_remove_count: summary.folder_remove_count,
        folder_undecided_count: summary.folder_undecided_count,
        effective_removal_file_count: summary.effective_removal_file_count,
        planned_removal_physical_item_count: summary.planned_removal_physical_item_count,
        planned_removal_bytes: summary.planned_removal_bytes.to_string(),
        remaining_physical_copy_count: summary.remaining_physical_copy_count,
        intact_folder_copy_count: summary.intact_folder_copy_count,
        rule_keep_count: summary.rule_keep_count,
        rule_remove_count: summary.rule_remove_count,
        active_rule_application_count: summary.active_rule_application_count,
    }
}

fn review_folder_group_summary_dto(
    summary: &ReviewFolderGroupSummary,
) -> ReviewFolderGroupSummaryDto {
    ReviewFolderGroupSummaryDto {
        folder_group_id: summary.folder_group_id,
        keep_count: summary.keep_count,
        remove_count: summary.remove_count,
        undecided_count: summary.undecided_count,
        intact_copy_count: summary.intact_copy_count,
    }
}

fn review_group_summary_dto(summary: &ReviewGroupSummary) -> ReviewGroupSummaryDto {
    ReviewGroupSummaryDto {
        group_id: summary.group_id,
        keep_count: summary.keep_count,
        remove_count: summary.remove_count,
        undecided_count: summary.undecided_count,
        remaining_physical_copy_count: summary.remaining_physical_copy_count,
    }
}

fn folder_group_dto(group: DuplicateFolderGroupResult) -> DuplicateFolderGroupDto {
    DuplicateFolderGroupDto {
        id: group.id,
        run_id: group.run_id,
        total_bytes: group.total_size.to_string(),
        descendant_file_count: group.file_count,
        copy_count: group.folder_count,
        representative_path: group.representative_path,
    }
}

fn folder_member_dto(member: DuplicateFolderMemberResult) -> DuplicateFolderMemberDto {
    DuplicateFolderMemberDto {
        id: member.id,
        group_id: member.group_id,
        path: member.path,
        decision: member.review_decision.as_str().to_owned(),
        decision_provenance: member.review_provenance,
        decision_at: member.review_decided_at,
    }
}

fn group_sort_name(field: DuplicateFileGroupSortField) -> &'static str {
    match field {
        DuplicateFileGroupSortField::RecoverableBytes => "recoverableBytes",
        DuplicateFileGroupSortField::GroupSize => "groupSize",
        DuplicateFileGroupSortField::CopyCount => "copyCount",
        DuplicateFileGroupSortField::RepresentativeName => "representativeName",
    }
}

fn warning_sort_name(field: RunWarningSortField) -> &'static str {
    match field {
        RunWarningSortField::Phase => "phase",
        RunWarningSortField::OccurrenceCount => "occurrenceCount",
        RunWarningSortField::Message => "message",
    }
}

fn warning_query_signature(
    run_id: i64,
    field: RunWarningSortField,
    direction: SortDirection,
    revision: i64,
    run_status: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        run_id,
        warning_sort_name(field),
        direction_name(direction),
        revision,
        run_status
    )
}

fn warning_snapshot_state(run_status: &str) -> &'static str {
    match run_status {
        "running" | "cancelling" => "active",
        "completed" | "cancelled" | "failed" | "interrupted" => "terminal",
        _ => "pending",
    }
}

fn diagnostic_log_metadata(path: Option<&Path>) -> Value {
    match path {
        Some(path) => json!({
            "state": "available",
            "locationKind": "local_file",
            "path": path.to_string_lossy(),
            "relationship": "supplemental_diagnostics_not_durable_warning_truth",
        }),
        None => json!({
            "state": "unavailable",
            "reason": "client_not_configured",
            "relationship": "supplemental_diagnostics_not_durable_warning_truth",
        }),
    }
}

fn path_match_name(path_match: DuplicateFilePathMatchMode) -> &'static str {
    match path_match {
        DuplicateFilePathMatchMode::Substring => "substring",
        DuplicateFilePathMatchMode::Exact => "exact",
    }
}

fn extension_match_name(extension_match: DuplicateFileExtensionMatchMode) -> &'static str {
    match extension_match {
        DuplicateFileExtensionMatchMode::AnyMember => "any",
        DuplicateFileExtensionMatchMode::AllMembers => "all",
    }
}

fn selected_root_facet_sort_name(field: DuplicateFileSelectedRootFacetSortField) -> &'static str {
    match field {
        DuplicateFileSelectedRootFacetSortField::MatchingGroupCount => "matchingGroupCount",
        DuplicateFileSelectedRootFacetSortField::Value => "value",
    }
}

fn drive_facet_sort_name(field: DuplicateFileDriveFacetSortField) -> &'static str {
    match field {
        DuplicateFileDriveFacetSortField::MatchingGroupCount => "matchingGroupCount",
        DuplicateFileDriveFacetSortField::Value => "value",
    }
}

fn member_sort_name(field: DuplicateFileMemberSortField) -> &'static str {
    match field {
        DuplicateFileMemberSortField::Path => "path",
        DuplicateFileMemberSortField::ModifiedTime => "modifiedTime",
        DuplicateFileMemberSortField::Size => "size",
    }
}

fn folder_group_sort_name(field: DuplicateFolderGroupSortField) -> &'static str {
    match field {
        DuplicateFolderGroupSortField::TotalBytes => "totalBytes",
        DuplicateFolderGroupSortField::CopyCount => "copyCount",
        DuplicateFolderGroupSortField::FileCount => "fileCount",
        DuplicateFolderGroupSortField::RepresentativePath => "representativePath",
    }
}

fn folder_member_sort_name(field: DuplicateFolderMemberSortField) -> &'static str {
    match field {
        DuplicateFolderMemberSortField::Path => "path",
    }
}

fn direction_name(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "ascending",
        SortDirection::Descending => "descending",
    }
}

struct ValidatedSession {
    name: String,
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
    cloud_policy: CloudPolicy,
    manual_location_exclusions: Vec<String>,
    registered_cloud_locations: Vec<RegisteredCloudLocation>,
    cloud_detection_status: CloudDetectionStatus,
}

#[allow(clippy::too_many_arguments)]
fn validate_session(
    name: String,
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
    cloud_policy: CloudPolicy,
    manual_location_exclusions: Vec<String>,
    registered_cloud_locations: Vec<RegisteredCloudLocation>,
    cloud_detection_status: CloudDetectionStatus,
) -> Result<ValidatedSession, ProtocolFailure> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(ProtocolFailure::new(
            "invalid_session",
            "Session name must contain 1 to 200 characters",
        )
        .with_details(json!({"field":"name"})));
    }
    if roots.is_empty() || roots.len() > 64 {
        return Err(ProtocolFailure::new(
            "invalid_session",
            "A session must contain 1 to 64 roots",
        )
        .with_details(json!({"field":"roots"})));
    }

    let manual_location_exclusions =
        validate_location_paths(manual_location_exclusions, 256, "manualLocationExclusions")?;
    if registered_cloud_locations.len() > 128 {
        return Err(ProtocolFailure::new(
            "invalid_session",
            "A session may contain at most 128 registered cloud locations",
        )
        .with_details(json!({"field":"registeredCloudLocations"})));
    }
    let mut validated_cloud_locations = Vec::with_capacity(registered_cloud_locations.len());
    for (location_index, location) in registered_cloud_locations.into_iter().enumerate() {
        let path = location.path.trim();
        let provider_id = location.provider_id.trim();
        let display_name = location.display_name.trim();
        if path.is_empty() || !Path::new(path).is_absolute() {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "Every registered cloud location must have an absolute path",
            )
            .with_details(
                json!({"field":"registeredCloudLocations", "locationIndex":location_index}),
            ));
        }
        if provider_id.chars().count() > 200 || display_name.chars().count() > 200 {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "Cloud provider identifiers and display names must not exceed 200 characters",
            )
            .with_details(
                json!({"field":"registeredCloudLocations", "locationIndex":location_index}),
            ));
        }
        let normalized_path = lexical_path(path);
        if !validated_cloud_locations
            .iter()
            .any(|existing: &RegisteredCloudLocation| {
                paths_equal(Path::new(&existing.path), Path::new(&normalized_path))
            })
        {
            validated_cloud_locations.push(RegisteredCloudLocation {
                path: normalized_path,
                provider_id: provider_id.to_owned(),
                display_name: if display_name.is_empty() {
                    "Cloud provider".to_owned()
                } else {
                    display_name.to_owned()
                },
            });
        }
    }

    let pre_io_exclusions = manual_location_exclusions
        .iter()
        .chain(
            (cloud_policy == CloudPolicy::ExcludeRegisteredRoots)
                .then_some(
                    validated_cloud_locations
                        .iter()
                        .map(|location| &location.path),
                )
                .into_iter()
                .flatten(),
        )
        .collect::<Vec<_>>();
    let mut normalized = Vec::with_capacity(roots.len());
    for (root_index, root) in roots.into_iter().enumerate() {
        let root = root.trim();
        if root.is_empty() || !Path::new(root).is_absolute() {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "Every root must be a non-empty absolute filesystem path",
            )
            .with_details(json!({"field":"roots", "rootIndex":root_index})));
        }
        let canonical = if pre_io_exclusions
            .iter()
            .any(|exclusion| path_is_within(Path::new(root), Path::new(exclusion)))
        {
            PathBuf::from(root)
        } else {
            fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root))
        };
        let value = canonical.to_string_lossy().into_owned();
        if !normalized
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&value))
        {
            normalized.push(value);
        }
    }
    normalized = super_duper_core::config::non_overlapping_directories(normalized);

    if ignore_patterns.len() > 512 {
        return Err(ProtocolFailure::new(
            "invalid_session",
            "A session may contain at most 512 ignore patterns",
        )
        .with_details(json!({"field":"ignorePatterns"})));
    }
    let mut validated_patterns = Vec::with_capacity(ignore_patterns.len());
    for pattern in ignore_patterns {
        let pattern = pattern.trim();
        if pattern.is_empty() || pattern.len() > 1024 {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "Ignore patterns must contain 1 to 1024 characters",
            )
            .with_details(json!({"field":"ignorePatterns"})));
        }
        Pattern::new(pattern).map_err(|error| {
            ProtocolFailure::new(
                "invalid_session",
                format!("Invalid ignore pattern '{pattern}': {error}"),
            )
            .with_details(json!({"field":"ignorePatterns", "pattern":pattern}))
        })?;
        if !validated_patterns
            .iter()
            .any(|existing| existing == pattern)
        {
            validated_patterns.push(pattern.to_owned());
        }
    }
    let encoded_size = serde_json::to_vec(&json!({
        "roots": &normalized,
        "ignorePatterns": &validated_patterns,
        "cloudPolicy": cloud_policy,
        "manualLocationExclusions": &manual_location_exclusions,
        "registeredCloudLocations": &validated_cloud_locations,
        "cloudDetectionStatus": cloud_detection_status,
    }))
    .map_err(|error| {
        ProtocolFailure::new(
            "invalid_session",
            format!("Session settings could not be encoded: {error}"),
        )
    })?
    .len();
    if encoded_size > MAXIMUM_FRAME_BYTES / 2 {
        return Err(ProtocolFailure::new(
            "invalid_session",
            "The combined session roots and ignore patterns are too large",
        ));
    }
    Ok(ValidatedSession {
        name,
        roots: normalized,
        ignore_patterns: validated_patterns,
        cloud_policy,
        manual_location_exclusions,
        registered_cloud_locations: validated_cloud_locations,
        cloud_detection_status,
    })
}

fn validate_location_paths(
    paths: Vec<String>,
    maximum: usize,
    field: &'static str,
) -> Result<Vec<String>, ProtocolFailure> {
    if paths.len() > maximum {
        return Err(ProtocolFailure::new(
            "invalid_session",
            format!("{field} may contain at most {maximum} paths"),
        )
        .with_details(json!({"field":field})));
    }
    let mut normalized = Vec::with_capacity(paths.len());
    for (path_index, path) in paths.into_iter().enumerate() {
        let path = path.trim();
        if path.is_empty() || !Path::new(path).is_absolute() {
            return Err(ProtocolFailure::new(
                "invalid_session",
                "Location exclusions must be non-empty absolute filesystem paths",
            )
            .with_details(json!({"field":field, "pathIndex":path_index})));
        }
        let path = lexical_path(path);
        if normalized
            .iter()
            .any(|existing: &String| path_is_within(Path::new(&path), Path::new(existing)))
        {
            continue;
        }
        normalized.retain(|existing| !path_is_within(Path::new(existing), Path::new(&path)));
        normalized.push(path);
    }
    Ok(normalized)
}

fn lexical_path(path: &str) -> String {
    let value = PathBuf::from(path).to_string_lossy().into_owned();
    #[cfg(windows)]
    return value.trim_end_matches(['\\', '/']).to_owned();
    #[cfg(not(windows))]
    value.trim_end_matches('/').to_owned()
}

#[cfg(windows)]
fn path_compare_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    let value = if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else if let Some(dos) = value.strip_prefix("\\\\?\\") {
        dos.to_owned()
    } else {
        value
    };
    value.trim_end_matches('\\').to_lowercase()
}

#[cfg(not(windows))]
fn path_compare_key(path: &Path) -> String {
    path.to_string_lossy().trim_end_matches('/').to_owned()
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    path_compare_key(left) == path_compare_key(right)
}

fn path_is_within(path: &Path, ancestor: &Path) -> bool {
    let path = path_compare_key(path);
    let ancestor = path_compare_key(ancestor);
    if path == ancestor {
        return true;
    }
    #[cfg(windows)]
    return path
        .strip_prefix(&ancestor)
        .is_some_and(|suffix| suffix.starts_with('\\'));
    #[cfg(not(windows))]
    path.strip_prefix(&ancestor)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn session_name_exists(
    db: &Database,
    name: &str,
    excluding_id: Option<i64>,
) -> Result<bool, ProtocolFailure> {
    let (sessions, _) = db
        .list_sessions(0, i64::MAX)
        .map_err(internal_database_error)?;
    Ok(sessions
        .iter()
        .any(|session| Some(session.id) != excluding_id && session.name.eq_ignore_ascii_case(name)))
}

fn get_session(db: &Database, session_id: i64) -> Result<ScanSession, ProtocolFailure> {
    db.get_session(session_id).map_err(|error| {
        if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
            ProtocolFailure::new(
                "session_not_found",
                format!("Session {session_id} was not found"),
            )
            .with_details(json!({"sessionId":session_id}))
        } else {
            internal_database_error(error)
        }
    })
}

fn get_run(db: &Database, run_id: i64) -> Result<ScanRun, ProtocolFailure> {
    db.get_scan_run(run_id).map_err(|error| {
        if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        } else {
            internal_database_error(error)
        }
    })
}

fn ensure_completed_result_run(db: &Database, run_id: i64) -> Result<(), ProtocolFailure> {
    let run = get_run(db, run_id)?;
    if run.status == "completed" {
        Ok(())
    } else {
        Err(ProtocolFailure::new(
            "invalid_state",
            "Duplicate-file results are available only for completed runs",
        )
        .with_details(json!({"runId":run.id,"status":run.status})))
    }
}

fn session_dto(session: ScanSession) -> Result<SessionDto, ProtocolFailure> {
    let cloud_policy = serde_json::from_value(Value::String(session.cloud_policy.clone()))
        .map_err(|error| {
            ProtocolFailure::new(
                "internal_error",
                format!("Stored session cloud policy is invalid: {error}"),
            )
        })?;
    let cloud_detection_status = serde_json::from_value(Value::String(
        session.cloud_detection_status.clone(),
    ))
    .map_err(|error| {
        ProtocolFailure::new(
            "internal_error",
            format!("Stored session cloud detection status is invalid: {error}"),
        )
    })?;
    Ok(SessionDto {
        id: session.id,
        name: session.name,
        roots: serde_json::from_str(&session.roots_json).map_err(|error| {
            ProtocolFailure::new(
                "internal_error",
                format!("Stored session roots are invalid: {error}"),
            )
        })?,
        ignore_patterns: serde_json::from_str(&session.ignore_patterns_json).map_err(|error| {
            ProtocolFailure::new(
                "internal_error",
                format!("Stored session ignore patterns are invalid: {error}"),
            )
        })?,
        cloud_policy,
        manual_location_exclusions: serde_json::from_str(&session.manual_location_exclusions_json)
            .map_err(|error| {
                ProtocolFailure::new(
                    "internal_error",
                    format!("Stored manual location exclusions are invalid: {error}"),
                )
            })?,
        registered_cloud_locations: serde_json::from_str(&session.registered_cloud_locations_json)
            .map_err(|error| {
                ProtocolFailure::new(
                    "internal_error",
                    format!("Stored registered cloud locations are invalid: {error}"),
                )
            })?,
        cloud_detection_status,
        created_at: session.created_at,
        updated_at: session.updated_at,
    })
}

fn run_dto(run: ScanRun) -> Result<RunDto, ProtocolFailure> {
    let parameters = RunParameters::from_json(&run.parameters_json).ok_or_else(|| {
        ProtocolFailure::new(
            "internal_error",
            format!("Run {} has invalid parameters", run.id),
        )
    })?;
    Ok(RunDto {
        id: run.id,
        session_id: run.session_id,
        parameters: RunParametersDto {
            roots: parameters.roots,
            ignore_patterns: parameters.ignore_patterns,
            directory_similarity_threshold_millis: parameters.directory_similarity_threshold_millis,
            cloud_policy: parameters.cloud_policy,
            manual_location_exclusions: parameters.manual_location_exclusions,
            registered_cloud_locations: parameters.registered_cloud_locations,
            cloud_detection_status: parameters.cloud_detection_status,
        },
        status: run.status,
        phase: run.phase,
        created_at: run.created_at,
        started_at: run.started_at,
        completed_at: run.completed_at,
        files_discovered: run.files_discovered,
        bytes_discovered: run.bytes_discovered.to_string(),
        files_hashed: run.files_hashed,
        duplicate_file_groups: run.duplicate_file_groups,
        duplicate_folder_groups: run.duplicate_folder_groups,
        wasted_bytes: run.wasted_bytes.to_string(),
        warning_count: run.warning_count,
        excluded_subtree_count: run.excluded_subtree_count,
        error_message: run.error_message,
        engine_version: run.engine_version,
    })
}

fn run_exclusion_dto(exclusion: RunExclusion) -> RunExclusionDto {
    RunExclusionDto {
        id: exclusion.id,
        run_id: exclusion.run_id,
        path: exclusion.path,
        reason_code: exclusion.reason_code,
        provider_id: exclusion.provider_id,
        provider_name: exclusion.provider_name,
        occurrence_count: exclusion.occurrence_count,
    }
}

fn run_warning_aggregate_dto(warning: RunWarningAggregate) -> RunWarningAggregateDto {
    RunWarningAggregateDto {
        id: warning.id,
        run_id: warning.run_id,
        phase: warning.phase,
        category: warning.category,
        code: warning.code,
        severity: warning.severity,
        message: warning.message,
        occurrence_count: warning.occurrence_count,
        examples: warning.examples,
    }
}

fn review_live_root_dto(root: &ReviewLiveRootState) -> Value {
    json!({
        "runId": root.run_id,
        "rootPath": root.root_path,
        "state": root.state,
        "dirtyRevision": root.dirty_revision,
        "reasonCode": root.reason_code,
        "dirtyAt": root.dirty_at,
        "reconciliationCursorFileId": root.reconciliation_cursor_file_id,
        "reconciledItemCount": root.reconciled_item_count,
        "updatedAt": root.updated_at,
        "reconciliationRequired": root.state == "dirty",
    })
}

fn internal_database_error(error: rusqlite::Error) -> ProtocolFailure {
    ProtocolFailure::new(
        "internal_error",
        format!("Database operation failed: {error}"),
    )
}

fn preference_rule_summary_dto(rule: &PreferenceRuleSummary) -> Value {
    json!({
        "id":rule.id,
        "name":rule.name,
        "kind":rule.kind,
        "revision":rule.revision,
        "rootCount":rule.root_count,
        "updatedAt":rule.updated_at,
    })
}

fn preference_rule_dto(rule: &PreferenceRule) -> Value {
    json!({
        "id":rule.id,
        "name":rule.name,
        "kind":rule.kind,
        "state":rule.state,
        "revision":rule.revision,
        "roots":rule.roots,
        "createdAt":rule.created_at,
        "updatedAt":rule.updated_at,
    })
}

fn preference_application_dto(application: &PreferenceRuleApplication) -> Value {
    json!({
        "id":application.id,
        "planId":application.plan_id,
        "runId":application.run_id,
        "ruleId":application.rule_id,
        "ruleRevision":application.rule_revision,
        "ruleName":application.rule_name,
        "ruleKind":application.rule_kind,
        "ruleRoots":application.rule_roots,
        "scopeKind":application.scope_kind,
        "scope":serde_json::from_str::<Value>(&application.scope_json).unwrap_or(Value::Null),
        "scopeSignature":application.scope_signature,
        "previewSignature":application.preview_signature,
        "sourceReviewRevision":application.source_review_revision,
        "appliedRevision":application.applied_revision,
        "state":application.state,
        "createdAt":application.created_at,
        "reversedAt":application.reversed_at,
        "summary":{
            "scopedGroupCount":application.summary.scoped_group_count,
            "applicableGroupCount":application.summary.applicable_group_count,
            "blockedGroupCount":application.summary.blocked_group_count,
            "ruleKeepPathCount":application.summary.rule_keep_path_count,
            "ruleRemovePathCount":application.summary.rule_remove_path_count,
            "ruleRemovePhysicalItemCount":application.summary.rule_remove_physical_item_count,
            "ruleRemoveBytes":application.summary.rule_remove_bytes.to_string(),
        }
    })
}

fn preference_application_summary_dto(application: &PreferenceRuleApplication) -> Value {
    json!({
        "id":application.id,
        "planId":application.plan_id,
        "runId":application.run_id,
        "ruleId":application.rule_id,
        "ruleRevision":application.rule_revision,
        "ruleName":application.rule_name,
        "ruleKind":application.rule_kind,
        "scopeKind":application.scope_kind,
        "sourceReviewRevision":application.source_review_revision,
        "appliedRevision":application.applied_revision,
        "state":application.state,
        "createdAt":application.created_at,
        "reversedAt":application.reversed_at,
        "summary":{
            "scopedGroupCount":application.summary.scoped_group_count,
            "applicableGroupCount":application.summary.applicable_group_count,
            "blockedGroupCount":application.summary.blocked_group_count,
            "ruleKeepPathCount":application.summary.rule_keep_path_count,
            "ruleRemovePathCount":application.summary.rule_remove_path_count,
            "ruleRemovePhysicalItemCount":application.summary.rule_remove_physical_item_count,
            "ruleRemoveBytes":application.summary.rule_remove_bytes.to_string(),
        }
    })
}

fn preference_preview_group_dto(group: &PreferencePreviewGroup) -> Value {
    json!({
        "groupId":group.group_id,
        "status":group.status.as_str(),
        "bestRank":group.best_rank,
        "preferredRoot":group.preferred_root,
        "tiedPreferredPathCount":group.tied_preferred_path_count,
        "proposedKeepPathCount":group.proposed_keep_path_count,
        "proposedRemovePathCount":group.proposed_remove_path_count,
        "proposedRemovePhysicalItemCount":group.proposed_remove_physical_item_count,
        "proposedRemoveBytes":group.proposed_remove_bytes.to_string(),
        "manualKeepCount":group.manual_keep_count,
        "manualRemoveCount":group.manual_remove_count,
        "explanationCode":group.explanation_code,
        "conflictFileId":group.conflict_file_id,
        "conflictFolderMemberId":group.conflict_folder_member_id,
    })
}

fn preference_preview_summary_dto(summary: &PreferencePreviewSummary) -> Value {
    json!({
        "scopedGroupCount":summary.scoped_group_count,
        "scopedLogicalPathCount":summary.scoped_logical_path_count,
        "scopedPhysicalItemCount":summary.scoped_physical_item_count,
        "scopedBytes":summary.scoped_bytes.to_string(),
        "affectedGroupCount":summary.affected_group_count,
        "blockedGroupCount":summary.blocked_group_count,
        "proposedKeepPathCount":summary.proposed_keep_path_count,
        "proposedRemovePathCount":summary.proposed_remove_path_count,
        "proposedRemovePhysicalItemCount":summary.proposed_remove_physical_item_count,
        "proposedRemoveBytes":summary.proposed_remove_bytes.to_string(),
        "manualKeepPathCount":summary.manual_keep_path_count,
        "manualRemovePathCount":summary.manual_remove_path_count,
        "tiedGroupCount":summary.tied_group_count,
        "noRankedRootGroupCount":summary.no_ranked_root_group_count,
        "missingRuleRootCount":summary.missing_rule_root_count,
        "overlapConflictCount":summary.overlap_conflict_count,
        "fileSurvivorConflictCount":summary.file_survivor_conflict_count,
        "folderSurvivorConflictCount":summary.folder_survivor_conflict_count,
    })
}

fn preflight_view_dto(view: &PreflightView) -> Value {
    let mut value = preflight_dto(&view.preflight);
    value["currentReviewRevision"] = json!(view.current_review_revision);
    value["isCurrent"] = json!(view.is_current);
    value
}

fn preflight_dto(preflight: &Preflight) -> Value {
    json!({
        "id":preflight.id,
        "operationId":preflight.operation_id,
        "runId":preflight.run_id,
        "planId":preflight.plan_id,
        "reviewRevision":preflight.review_revision,
        "snapshotSignature":preflight.snapshot_signature,
        "status":preflight.status,
        "logicalRemovalCount":preflight.summary.logical_removal_count,
        "physicalRemovalCount":preflight.summary.physical_removal_count,
        "folderRemovalCount":preflight.summary.folder_removal_count,
        "affectedGroupCount":preflight.summary.affected_group_count,
        "plannedRemovalBytes":preflight.summary.planned_removal_bytes.to_string(),
        "totalItemCount":preflight.summary.total_item_count,
        "processedItemCount":preflight.summary.processed_item_count,
        "readyCount":preflight.summary.ready_count,
        "changedCount":preflight.summary.changed_count,
        "missingCount":preflight.summary.missing_count,
        "unavailableCount":preflight.summary.unavailable_count,
        "conflictCount":preflight.summary.conflict_count,
        "createdAt":preflight.created_at,
        "startedAt":preflight.started_at,
        "completedAt":preflight.completed_at,
        "errorCode":preflight.error_code,
        "errorDetail":preflight.error_detail,
    })
}

fn preflight_item_dto(item: &PreflightItem) -> Value {
    json!({
        "id":item.id,
        "preflightId":item.preflight_id,
        "ordinal":item.ordinal,
        "targetKind":item.target_kind,
        "targetRole":item.target_role,
        "groupId":item.group_id,
        "folderGroupId":item.folder_group_id,
        "folderMemberId":item.folder_member_id,
        "snapshotFileId":item.snapshot_file_id,
        "snapshotDirectoryId":item.snapshot_directory_id,
        "path":item.snapshot_path,
        "outcome":item.outcome,
        "reasonCode":item.reason_code,
        "observedFileSize":item.observed_file_size.map(|value| value.to_string()),
        "observedLastModified":item.observed_last_modified,
        "osError":item.os_error,
        "observedAt":item.observed_at,
        "sourceCount":item.source_count,
    })
}

fn recycle_operation_view_dto(view: &RecycleOperationView) -> Value {
    let mut value = recycle_operation_dto(&view.operation);
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "currentReviewRevision".to_owned(),
            json!(view.current_review_revision),
        );
        object.insert("isCurrent".to_owned(), json!(view.is_current));
    }
    value
}

fn recycle_operation_dto(operation: &RecycleOperation) -> Value {
    json!({
        "id": operation.id,
        "operationId": operation.operation_id,
        "runId": operation.run_id,
        "planId": operation.plan_id,
        "preflightId": operation.preflight_id,
        "reviewRevision": operation.review_revision,
        "preflightSnapshotSignature": operation.preflight_snapshot_signature,
        "intentSignature": operation.intent_signature,
        "policyVersion": operation.policy_version,
        "status": operation.status,
        "logicalRemovalCount": operation.summary.logical_removal_count,
        "shellItemCount": operation.summary.shell_item_count,
        "physicalItemCount": operation.summary.physical_item_count,
        "folderItemCount": operation.summary.folder_item_count,
        "affectedGroupCount": operation.summary.affected_group_count,
        "plannedRemovalBytes": operation.summary.planned_removal_bytes.to_string(),
        "affectedLocationCount": operation.summary.affected_location_count,
        "exclusionCount": operation.summary.exclusion_count,
        "eligibleCount": operation.summary.eligible_count,
        "nonRecyclableCount": operation.summary.non_recyclable_count,
        "pendingEligibilityCount": operation.summary.pending_eligibility_count,
        "recycledCount": operation.summary.recycled_count,
        "failedCount": operation.summary.failed_count,
        "cancelledCount": operation.summary.cancelled_count,
        "unknownCount": operation.summary.unknown_count,
        "pendingResultCount": operation.summary.pending_result_count,
        "preparedAt": operation.prepared_at,
        "confirmationSignature": operation.confirmation_signature,
        "confirmationExpiresAt": operation.confirmation_expires_at,
        "submittedAt": operation.submitted_at,
        "completedAt": operation.completed_at,
        "cancellationRequested": operation.cancellation_requested,
        "errorCode": operation.error_code,
        "errorDetail": operation.error_detail,
    })
}

fn recycle_operation_item_dto(item: &RecycleOperationItem) -> Value {
    json!({
        "id": item.id,
        "recycleOperationId": item.recycle_operation_id,
        "batchId": item.batch_id,
        "ordinal": item.ordinal,
        "preflightItemId": item.preflight_item_id,
        "preflightSourceId": item.preflight_source_id,
        "targetKind": item.target_kind,
        "path": item.snapshot_path,
        "groupId": item.group_id,
        "folderGroupId": item.folder_group_id,
        "folderMemberId": item.folder_member_id,
        "snapshotFileId": item.snapshot_file_id,
        "snapshotDirectoryId": item.snapshot_directory_id,
        "plannedBytes": item.planned_bytes.to_string(),
        "eligibilityStatus": item.eligibility_status,
        "eligibilityCode": item.eligibility_code,
        "resultStatus": item.result_status,
        "resultCode": item.result_code,
        "shellHresult": item.shell_hresult,
        "recycledItemPresent": item.recycled_item_present,
        "resultAt": item.result_at,
    })
}

fn recycle_operation_batch_dto(batch: &RecycleOperationBatch) -> Value {
    let items = batch
        .items
        .iter()
        .map(|item| {
            let mut dto = recycle_operation_item_dto(item);
            let object = dto
                .as_object_mut()
                .expect("recycle operation item DTO is an object");
            object.insert(
                "snapshotFileIdentity".to_owned(),
                json!(item.snapshot_file_identity),
            );
            object.insert(
                "snapshotFileSize".to_owned(),
                json!(item.snapshot_file_size.map(|value| value.to_string())),
            );
            object.insert(
                "snapshotLastModified".to_owned(),
                json!(item.snapshot_last_modified),
            );
            dto
        })
        .collect::<Vec<_>>();
    json!({
        "id": batch.id,
        "recycleOperationId": batch.recycle_operation_id,
        "ordinal": batch.ordinal,
        "itemSignature": batch.item_signature,
        "status": batch.status,
        "admissionExpiresAt": batch.admission_expires_at,
        "shellAttemptId": batch.shell_attempt_id,
        "startedAt": batch.started_at,
        "reportedAt": batch.reported_at,
        "items": items,
    })
}

fn recovery_review_summary_dto(summary: &RecoveryReviewSummary) -> Value {
    json!({
        "recycleOperationId": summary.recycle_operation_id,
        "state": summary.state.as_str(),
        "unknownItemCount": summary.unknown_item_count,
        "observedItemCount": summary.observed_item_count,
    })
}

fn recovery_review_observation_dto(observation: &RecoveryReviewObservation) -> Value {
    json!({
        "id": observation.id,
        "requestId": observation.request_id,
        "recycleOperationId": observation.recycle_operation_id,
        "itemId": observation.item_id,
        "observation": observation.observation.as_str(),
        "observedAt": observation.observed_at,
        "note": observation.note,
        "evidenceVersion": observation.evidence_version,
        "supersedesObservationId": observation.supersedes_observation_id,
        "correctionReason": observation.correction_reason,
        "createdAt": observation.created_at,
        "supersededByObservationId": observation.superseded_by_observation_id,
        "isCurrent": observation.is_current,
    })
}

fn recovery_review_error(error: RecoveryReviewError) -> ProtocolFailure {
    match error {
        RecoveryReviewError::Database(error) => internal_database_error(error),
        RecoveryReviewError::InvalidRequest { message } => {
            ProtocolFailure::new("invalid_request", message)
        }
        RecoveryReviewError::OperationNotFound { operation_id } => ProtocolFailure::new(
            "recycle_operation_not_found",
            format!("Recycle operation {operation_id} was not found"),
        )
        .with_details(json!({"recycleOperationId": operation_id})),
        RecoveryReviewError::InvalidOperationState {
            operation_id,
            status,
        } => ProtocolFailure::new(
            "recovery_review_invalid_state",
            "Recovery review is available only for a recovery-required operation",
        )
        .with_details(json!({"recycleOperationId": operation_id, "status": status})),
        RecoveryReviewError::ItemNotFound {
            operation_id,
            item_id,
        } => ProtocolFailure::new(
            "recycle_operation_item_not_found",
            "The recovery-review item does not belong to this recycle operation",
        )
        .with_details(json!({"recycleOperationId": operation_id, "itemId": item_id})),
        RecoveryReviewError::NonUnknownItem {
            operation_id,
            item_id,
            result_status,
        } => ProtocolFailure::new(
            "recovery_review_item_not_unknown",
            "Only an immutable unknown operation item can receive a recovery observation",
        )
        .with_details(json!({
            "recycleOperationId": operation_id,
            "itemId": item_id,
            "resultStatus": result_status,
        })),
        RecoveryReviewError::ObservationNotFound { observation_id } => ProtocolFailure::new(
            "recovery_review_observation_not_found",
            format!("Recovery observation {observation_id} was not found"),
        )
        .with_details(json!({"observationId": observation_id})),
        RecoveryReviewError::SupersessionConflict {
            observation_id,
            item_id,
        } => ProtocolFailure::new(
            "recovery_review_supersession_conflict",
            "The prior observation is not the current observation for this item",
        )
        .with_details(json!({"observationId": observation_id, "itemId": item_id})),
        RecoveryReviewError::CurrentObservationExists {
            item_id,
            observation_id,
        } => ProtocolFailure::new(
            "recovery_review_current_observation_exists",
            "The item already has a current observation; supersede it explicitly",
        )
        .with_details(json!({"itemId": item_id, "observationId": observation_id})),
        RecoveryReviewError::IdempotencyConflict { request_id } => ProtocolFailure::new(
            "idempotency_conflict",
            "The request ID was already used with another recovery-review payload",
        )
        .with_details(json!({"requestId": request_id})),
    }
}

fn recycle_operation_error(error: RecycleOperationError) -> ProtocolFailure {
    match error {
        RecycleOperationError::Database(error) => internal_database_error(error),
        RecycleOperationError::InvalidRequest { message } => {
            ProtocolFailure::new("invalid_request", message)
        }
        RecycleOperationError::RunNotFound { run_id } => {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        }
        RecycleOperationError::PreflightNotFound { preflight_id } => ProtocolFailure::new(
            "preflight_not_found",
            format!("Preflight {preflight_id} was not found"),
        )
        .with_details(json!({"preflightId":preflight_id})),
        RecycleOperationError::PreflightNotCompleted {
            preflight_id,
            status,
        } => ProtocolFailure::new(
            "operation_preflight_incomplete",
            "Recycle operation preparation requires a completed preflight",
        )
        .with_details(json!({"preflightId":preflight_id,"status":status})),
        RecycleOperationError::LatestPreflightRequired {
            preflight_id,
            run_id,
        } => ProtocolFailure::new(
            "operation_preflight_superseded",
            "A newer preflight generation exists; inspect it before continuing",
        )
        .with_details(json!({"preflightId":preflight_id,"runId":run_id})),
        RecycleOperationError::StaleReviewRevision { expected, current } => ProtocolFailure::new(
            "review_generation_conflict",
            "The review plan changed; run preflight again",
        )
        .with_details(json!({"expectedRevision":expected,"currentRevision":current})),
        RecycleOperationError::PreflightExpired { preflight_id } => ProtocolFailure::new(
            "operation_preflight_expired",
            "The completed preflight is outside the preparation freshness lease",
        )
        .with_details(json!({"preflightId":preflight_id,"freshnessSeconds":300})),
        RecycleOperationError::IneligiblePreflight {
            preflight_id,
            reason,
        } => ProtocolFailure::new(
            "operation_preflight_ineligible",
            "The complete reviewed plan is not eligible for operation preparation",
        )
        .with_details(json!({"preflightId":preflight_id,"reason":reason})),
        RecycleOperationError::IdempotencyConflict { operation_id } => ProtocolFailure::new(
            "idempotency_conflict",
            "The operation ID was already used with another recycle-operation payload",
        )
        .with_details(json!({"operationId":operation_id})),
        RecycleOperationError::NotFound { operation_id } => ProtocolFailure::new(
            "recycle_operation_not_found",
            format!("Recycle operation {operation_id} was not found"),
        )
        .with_details(json!({"recycleOperationId":operation_id})),
        RecycleOperationError::InvalidState {
            operation_id,
            status,
        } => ProtocolFailure::new(
            "recycle_operation_invalid_state",
            "The recycle operation cannot perform this transition",
        )
        .with_details(json!({"recycleOperationId":operation_id,"status":status})),
        RecycleOperationError::OperationLocked {
            run_id,
            operation_id,
        } => ProtocolFailure::new(
            "recycle_operation_locked",
            "Another durable recycle operation locks this reviewed plan",
        )
        .with_details(json!({"runId":run_id,"recycleOperationId":operation_id})),
        RecycleOperationError::ConfirmationExpired { operation_id } => ProtocolFailure::new(
            "recycle_operation_confirmation_expired",
            "The final confirmation lease expired; run preflight again",
        )
        .with_details(json!({"recycleOperationId":operation_id})),
        RecycleOperationError::SubmissionExpired { operation_id } => ProtocolFailure::new(
            "recycle_operation_submission_expired",
            "The provisional batch-admission lease expired before Shell work could start",
        )
        .with_details(json!({"recycleOperationId":operation_id,"freshnessSeconds":30})),
        RecycleOperationError::ItemNotFound {
            operation_id,
            item_id,
        } => ProtocolFailure::new(
            "recycle_operation_item_not_found",
            "The reported item does not belong to this recycle operation",
        )
        .with_details(json!({"recycleOperationId":operation_id,"itemId":item_id})),
        RecycleOperationError::BatchNotFound {
            operation_id,
            batch_id,
        } => ProtocolFailure::new(
            "recycle_operation_batch_not_found",
            "The reported batch does not belong to this recycle operation",
        )
        .with_details(json!({"recycleOperationId":operation_id,"batchId":batch_id})),
        RecycleOperationError::AdmissionFailed {
            operation_id,
            item_id,
            reason_code,
        } => ProtocolFailure::new(
            "recycle_operation_admission_failed",
            format!("Fresh admission rejected item {item_id}: {reason_code}"),
        )
        .with_details(json!({
            "recycleOperationId": operation_id,
            "itemId": item_id,
            "reasonCode": reason_code,
        })),
        RecycleOperationError::AdmissionValidationFailed {
            operation_id,
            message,
        } => ProtocolFailure::new("recycle_operation_admission_unavailable", message)
            .with_details(json!({"recycleOperationId": operation_id})),
    }
}

fn preflight_error(error: PreflightError) -> ProtocolFailure {
    match error {
        PreflightError::Database(error) => internal_database_error(error),
        PreflightError::Review(error) => review_error(error),
        PreflightError::RunNotFound { run_id } => {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        }
        PreflightError::RunNotCompleted { run_id, status } => ProtocolFailure::new(
            "invalid_state",
            "Preflight is available only for completed runs",
        )
        .with_details(json!({"runId":run_id,"status":status})),
        PreflightError::PlanNotFound { run_id } => ProtocolFailure::new(
            "preflight_empty",
            "Review at least one removal before starting preflight",
        )
        .with_details(json!({"runId":run_id})),
        PreflightError::StaleReviewRevision { expected, current } => ProtocolFailure::new(
            "review_generation_conflict",
            "The review plan changed; reload it before starting preflight",
        )
        .with_details(json!({"expectedRevision":expected,"currentRevision":current})),
        PreflightError::EmptyPlan => ProtocolFailure::new(
            "preflight_empty",
            "Review at least one removal before starting preflight",
        ),
        PreflightError::IdempotencyConflict { operation_id } => ProtocolFailure::new(
            "operation_conflict",
            "The operationId was already used for another preflight payload",
        )
        .with_details(json!({"operationId":operation_id})),
        PreflightError::NotFound { preflight_id } => ProtocolFailure::new(
            "preflight_not_found",
            format!("Preflight {preflight_id} was not found"),
        )
        .with_details(json!({"preflightId":preflight_id})),
        PreflightError::InvalidState {
            preflight_id,
            status,
        } => ProtocolFailure::new(
            "preflight_not_cancellable",
            format!("Preflight {preflight_id} is already {status}"),
        )
        .with_details(json!({"preflightId":preflight_id,"status":status})),
        PreflightError::SnapshotConflict { message } => ProtocolFailure::new(
            "preflight_snapshot_conflict",
            "The reviewed plan could not be frozen safely",
        )
        .with_details(json!({"reason":message})),
        PreflightError::InvalidRequest { message } => {
            ProtocolFailure::new("invalid_request", message)
        }
        PreflightError::OperationLocked {
            run_id,
            operation_id,
        } => ProtocolFailure::new(
            "recycle_operation_locked",
            "A durable recycle operation locks this reviewed plan",
        )
        .with_details(json!({"runId":run_id,"recycleOperationId":operation_id})),
    }
}

fn preference_error(error: PreferenceError) -> ProtocolFailure {
    match error {
        PreferenceError::Review(error) => review_error(error),
        PreferenceError::Database(error) => internal_database_error(error),
        PreferenceError::Serialization(error) => ProtocolFailure::new(
            "internal_error",
            format!("Rule serialization failed: {error}"),
        ),
        PreferenceError::RunNotFound { run_id } => {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        }
        PreferenceError::RunNotCompleted { run_id, status } => ProtocolFailure::new(
            "invalid_state",
            "Preference preview is available only for completed runs",
        )
        .with_details(json!({"runId":run_id,"status":status})),
        PreferenceError::RuleNotFound { rule_id } => ProtocolFailure::new(
            "preference_rule_not_found",
            format!("Preference rule {rule_id} was not found"),
        )
        .with_details(json!({"ruleId":rule_id})),
        PreferenceError::RuleArchived { rule_id } => ProtocolFailure::new(
            "invalid_state",
            "Archived preference rules cannot be previewed or edited",
        )
        .with_details(json!({"ruleId":rule_id,"state":"archived"})),
        PreferenceError::StaleRuleRevision {
            rule_id,
            expected,
            current,
        } => ProtocolFailure::new(
            "preference_rule_generation_conflict",
            "The preference rule changed; reload it before continuing",
        )
        .with_details(json!({
            "ruleId":rule_id,
            "expectedRevision":expected,
            "currentRevision":current
        })),
        PreferenceError::StaleReviewRevision { expected, current } => ProtocolFailure::new(
            "review_generation_conflict",
            "Manual review decisions changed; rerun the preview",
        )
        .with_details(json!({"expectedRevision":expected,"currentRevision":current})),
        PreferenceError::IdempotencyConflict { operation_id } => ProtocolFailure::new(
            "idempotency_conflict",
            "The operationId was already used for another preference-rule command payload",
        )
        .with_details(json!({"operationId":operation_id})),
        PreferenceError::DuplicateName { name } => ProtocolFailure::new(
            "preference_rule_name_conflict",
            "Another preference rule already uses this name",
        )
        .with_details(json!({"name":name})),
        PreferenceError::InvalidSelectedGroup { run_id, group_id } => ProtocolFailure::new(
            "invalid_scope",
            "A selected duplicate set does not belong to the completed run",
        )
        .with_details(json!({"runId":run_id,"groupId":group_id})),
        PreferenceError::InvalidRule { message } => {
            ProtocolFailure::new("invalid_request", message)
        }
        PreferenceError::PreviewTooComplex {
            scoped_group_count,
            maximum_group_count,
            scoped_logical_path_count,
            maximum_logical_path_count,
        } => ProtocolFailure::new(
            "preview_too_complex",
            "The preference preview scope exceeds this protocol version's bounded limits",
        )
        .with_details(json!({
            "scopedGroupCount": scoped_group_count,
            "maximumGroupCount": maximum_group_count,
            "scopedLogicalPathCount": scoped_logical_path_count,
            "maximumLogicalPathCount": maximum_logical_path_count,
        })),
        PreferenceError::PreviewConflict => ProtocolFailure::new(
            "preference_preview_conflict",
            "The preview no longer matches this rule application request; run Preview again",
        ),
        PreferenceError::ApplicationEmpty => ProtocolFailure::new(
            "rule_application_empty",
            "The preview contains no applicable rule decisions",
        ),
        PreferenceError::ApplicationOverlap {
            file_id,
            application_id,
        } => ProtocolFailure::new(
            "rule_application_overlap",
            "Another active rule application already owns one of these decisions",
        )
        .with_details(json!({"fileId":file_id,"applicationId":application_id})),
        PreferenceError::ApplicationNotFound {
            run_id,
            application_id,
        } => ProtocolFailure::new(
            "rule_application_not_found",
            format!("Rule application {application_id} was not found in run {run_id}"),
        )
        .with_details(json!({"runId":run_id,"applicationId":application_id})),
        PreferenceError::ApplicationAlreadyReversed { application_id } => ProtocolFailure::new(
            "rule_application_already_reversed",
            "This rule application has already been reversed",
        )
        .with_details(json!({"applicationId":application_id})),
        PreferenceError::OperationLocked {
            run_id,
            operation_id,
        } => ProtocolFailure::new(
            "recycle_operation_locked",
            "A durable recycle operation locks this reviewed plan",
        )
        .with_details(json!({"runId":run_id,"recycleOperationId":operation_id})),
    }
}

fn review_error(error: ReviewError) -> ProtocolFailure {
    match error {
        ReviewError::Database(error) => internal_database_error(error),
        ReviewError::RunNotFound { run_id } => {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        }
        ReviewError::RunNotCompleted { run_id, status } => ProtocolFailure::new(
            "invalid_state",
            "Review decisions are available only for completed runs",
        )
        .with_details(json!({"runId":run_id,"status":status})),
        ReviewError::GroupNotFound { run_id, group_id } => ProtocolFailure::new(
            "duplicate_group_not_found",
            format!("Duplicate file group {group_id} was not found in run {run_id}"),
        )
        .with_details(json!({"runId":run_id,"groupId":group_id})),
        ReviewError::MemberNotFound {
            run_id,
            group_id,
            file_id,
        } => ProtocolFailure::new(
            "review_member_not_found",
            format!("File {file_id} is not a member of duplicate group {group_id}"),
        )
        .with_details(json!({"runId":run_id,"groupId":group_id,"fileId":file_id})),
        ReviewError::StaleRevision { expected, actual } => ProtocolFailure::new(
            "review_generation_conflict",
            format!("Review revision {expected} is stale; current revision is {actual}"),
        )
        .with_details(json!({"expectedRevision":expected,"currentRevision":actual})),
        ReviewError::IdempotencyConflict { operation_id } => ProtocolFailure::new(
            "idempotency_conflict",
            "The operationId was already used for a different review command",
        )
        .with_details(json!({"operationId":operation_id})),
        ReviewError::UnsafeRemoval { group_id, file_id } => ProtocolFailure::new(
            "unsafe_review_decision",
            "At least one independently accessible physical copy must remain in the duplicate set",
        )
        .with_details(json!({"groupId":group_id,"fileId":file_id,"remainingPhysicalCopies":0})),
        ReviewError::FolderGroupNotFound {
            run_id,
            folder_group_id,
        } => ProtocolFailure::new(
            "duplicate_folder_group_not_found",
            format!("Exact-folder group {folder_group_id} was not found in run {run_id}"),
        )
        .with_details(json!({"runId":run_id,"folderGroupId":folder_group_id})),
        ReviewError::FolderMemberNotFound {
            run_id,
            folder_group_id,
            folder_member_id,
        } => ProtocolFailure::new(
            "review_folder_member_not_found",
            format!("Folder copy {folder_member_id} is not a member of exact-folder group {folder_group_id}"),
        )
        .with_details(json!({
            "runId":run_id,
            "folderGroupId":folder_group_id,
            "folderMemberId":folder_member_id
        })),
        ReviewError::Overlap {
            first_kind,
            first_id,
            second_kind,
            second_id,
        } => ProtocolFailure::new(
            "review_overlap_conflict",
            "The requested review decision overlaps an existing file or folder decision",
        )
        .with_details(json!({
            "firstKind":first_kind,
            "firstId":first_id,
            "secondKind":second_kind,
            "secondId":second_id
        })),
        ReviewError::UnsafePhysicalRemoval { duplicate_group_id } => ProtocolFailure::new(
            "unsafe_review_decision",
            "At least one independently accessible physical copy must remain in every duplicate set",
        )
        .with_details(json!({
            "groupId":duplicate_group_id,
            "remainingPhysicalCopies":0
        })),
        ReviewError::UnsafeFolderRemoval { folder_group_id } => ProtocolFailure::new(
            "unsafe_folder_review_decision",
            "At least one intact independently accessible folder copy must remain in every exact-folder set",
        )
        .with_details(json!({
            "folderGroupId":folder_group_id,
            "remainingIntactCopies":0
        })),
        ReviewError::OperationLocked { run_id, operation_id } => ProtocolFailure::new(
            "recycle_operation_locked",
            "A durable recycle operation locks this reviewed plan",
        )
        .with_details(json!({"runId":run_id,"recycleOperationId":operation_id})),
        ReviewError::LiveStateConflict {
            file_id,
            state,
            decision,
        } => ProtocolFailure::new(
            "review_live_state_conflict",
            "The file is not currently validated as present",
        )
        .with_details(json!({"fileId":file_id,"state":state,"decision":decision})),
    }
}

fn review_live_validation_error(error: ReviewLiveValidationError) -> ProtocolFailure {
    match error {
        ReviewLiveValidationError::Database(error) => internal_database_error(error),
        ReviewLiveValidationError::InvalidRequest { message } => {
            ProtocolFailure::new("invalid_request", message)
        }
        ReviewLiveValidationError::RunNotFound { run_id } => {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        }
        ReviewLiveValidationError::RunNotCompleted { run_id, status } => ProtocolFailure::new(
            "invalid_state",
            "Validation is available only for completed runs",
        )
        .with_details(json!({"runId":run_id,"status":status})),
        ReviewLiveValidationError::GroupNotFound { run_id, group_id } => ProtocolFailure::new(
            "duplicate_group_not_found",
            "The selected duplicate set was not found",
        )
        .with_details(json!({"runId":run_id,"groupId":group_id})),
        ReviewLiveValidationError::MemberNotFound {
            run_id,
            group_id,
            file_id,
        } => ProtocolFailure::new(
            "review_member_not_found",
            "A requested visible-page file is not in the selected duplicate set",
        )
        .with_details(json!({"runId":run_id,"groupId":group_id,"fileId":file_id})),
        ReviewLiveValidationError::StaleRevision { expected, actual } => ProtocolFailure::new(
            "review_generation_conflict",
            "Review choices changed before validation committed",
        )
        .with_details(json!({"expectedRevision":expected,"currentRevision":actual})),
        ReviewLiveValidationError::IdempotencyConflict { operation_id } => ProtocolFailure::new(
            "idempotency_conflict",
            "The operationId was already used for another validation request",
        )
        .with_details(json!({"operationId":operation_id})),
        ReviewLiveValidationError::InvalidRunParameters { run_id } => ProtocolFailure::new(
            "invalid_run_snapshot",
            "The immutable run parameter snapshot could not be decoded",
        )
        .with_details(json!({"runId":run_id})),
    }
}

fn review_live_hint_error(error: ReviewLiveHintError) -> ProtocolFailure {
    match error {
        ReviewLiveHintError::Database(error) => internal_database_error(error),
        ReviewLiveHintError::InvalidRequest { message } => {
            ProtocolFailure::new("invalid_request", message)
        }
        ReviewLiveHintError::RunNotFound { run_id } => {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        }
        ReviewLiveHintError::RunNotCompleted { run_id, status } => ProtocolFailure::new(
            "invalid_state",
            "Live hints are available only for completed runs",
        )
        .with_details(json!({"runId":run_id,"status":status})),
        ReviewLiveHintError::RootNotFound { run_id, root_path } => ProtocolFailure::new(
            "review_root_not_found",
            "The hinted path is not under an immutable selected root for this run",
        )
        .with_details(json!({"runId":run_id,"rootPath":root_path})),
        ReviewLiveHintError::InvalidRunParameters { run_id } => ProtocolFailure::new(
            "invalid_run_snapshot",
            "The immutable run parameter snapshot could not be decoded",
        )
        .with_details(json!({"runId":run_id})),
    }
}

fn review_live_root_error(error: ReviewLiveRootError) -> ProtocolFailure {
    match error {
        ReviewLiveRootError::Database(error) => internal_database_error(error),
        ReviewLiveRootError::InvalidRequest { message } => {
            ProtocolFailure::new("invalid_request", message)
        }
        ReviewLiveRootError::RunNotFound { run_id } => {
            ProtocolFailure::new("run_not_found", format!("Run {run_id} was not found"))
                .with_details(json!({"runId":run_id}))
        }
        ReviewLiveRootError::RunNotCompleted { run_id, status } => ProtocolFailure::new(
            "invalid_state",
            "Dirty-root reconciliation is available only for completed runs",
        )
        .with_details(json!({"runId":run_id,"status":status})),
        ReviewLiveRootError::RootNotFound { run_id, root_path } => ProtocolFailure::new(
            "review_root_not_found",
            "The requested path is not an immutable selected root for this run",
        )
        .with_details(json!({"runId":run_id,"rootPath":root_path})),
        ReviewLiveRootError::RootNotDirty { run_id, root_path } => ProtocolFailure::new(
            "review_root_not_dirty",
            "The requested root no longer requires reconciliation",
        )
        .with_details(json!({"runId":run_id,"rootPath":root_path})),
        ReviewLiveRootError::StaleDirtyRevision { expected, actual } => ProtocolFailure::new(
            "dirty_generation_conflict",
            "Another overflow changed the dirty root before reconciliation committed",
        )
        .with_details(json!({"expectedDirtyRevision":expected,"currentDirtyRevision":actual})),
        ReviewLiveRootError::StaleReviewRevision { expected, actual } => ProtocolFailure::new(
            "review_generation_conflict",
            "Review choices changed before root reconciliation committed",
        )
        .with_details(json!({"expectedRevision":expected,"currentRevision":actual})),
        ReviewLiveRootError::StaleReconciliationCursor => ProtocolFailure::new(
            "dirty_reconciliation_conflict",
            "Another bounded request advanced this dirty root; reload its current state",
        ),
        ReviewLiveRootError::IdempotencyConflict { operation_id } => ProtocolFailure::new(
            "idempotency_conflict",
            "The operationId was already used for another dirty-root request",
        )
        .with_details(json!({"operationId":operation_id})),
        ReviewLiveRootError::InvalidRunParameters { run_id } => ProtocolFailure::new(
            "invalid_run_snapshot",
            "The immutable run parameter snapshot could not be decoded",
        )
        .with_details(json!({"runId":run_id})),
    }
}

fn request_id(object: &Map<String, Value>) -> Result<&str, WorkerError> {
    object
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            WorkerError::FatalProtocol("request frame has no correlatable id".to_owned())
        })
}

fn serialize_success(id: &str, result: Value) -> Result<String, WorkerError> {
    let response = serialize_response_unbounded(ResponseEnvelope {
        message_type: "response",
        id: id.to_owned(),
        ok: true,
        result: Some(result),
        error: None,
    })?;
    if response.len() <= MAXIMUM_FRAME_BYTES {
        return Ok(response);
    }
    serialize_failure(
        id,
        ProtocolFailure {
            code: "invalid_request",
            message: "The response exceeds the protocol frame limit; request a smaller page"
                .to_owned(),
            retryable: true,
            details: json!({"maximumFrameBytes":MAXIMUM_FRAME_BYTES}),
        },
    )
}

fn serialize_failure(id: &str, failure: ProtocolFailure) -> Result<String, WorkerError> {
    serialize_response(ResponseEnvelope {
        message_type: "response",
        id: id.to_owned(),
        ok: false,
        result: None,
        error: Some(StructuredError {
            code: failure.code.to_owned(),
            message: failure.message,
            retryable: failure.retryable,
            details: failure.details,
        }),
    })
}

fn serialize_response(response: ResponseEnvelope) -> Result<String, WorkerError> {
    let frame = serialize_response_unbounded(response)?;
    if frame.len() > MAXIMUM_FRAME_BYTES {
        return Err(WorkerError::FatalProtocol(
            "structured error exceeds the protocol frame limit".to_owned(),
        ));
    }
    Ok(frame)
}

fn serialize_response_unbounded(response: ResponseEnvelope) -> Result<String, WorkerError> {
    serde_json::to_string(&response).map_err(|error| {
        WorkerError::FatalProtocol(format!("could not serialize protocol response: {error}"))
    })
}

fn default_page_size() -> i64 {
    DEFAULT_PAGE_SIZE
}

fn default_result_page_size() -> i64 {
    DEFAULT_RESULT_PAGE_SIZE
}

fn default_performance_history_page_size() -> i64 {
    PERFORMANCE_HISTORY_PAGE_SIZE
}

fn default_group_sort_field() -> String {
    "recoverableBytes".to_owned()
}

fn default_warning_sort_field() -> String {
    "occurrenceCount".to_owned()
}

fn default_selected_root_facet_sort_field() -> String {
    "matchingGroupCount".to_owned()
}

fn default_drive_facet_sort_field() -> String {
    "matchingGroupCount".to_owned()
}

fn default_folder_group_sort_field() -> String {
    "totalBytes".to_owned()
}

fn default_member_sort_field() -> String {
    "path".to_owned()
}

fn default_path_match() -> String {
    "substring".to_owned()
}

fn default_extension_match() -> String {
    "any".to_owned()
}

fn default_descending() -> String {
    "descending".to_owned()
}

fn default_ascending() -> String {
    "ascending".to_owned()
}

fn default_minimum_size() -> String {
    "0".to_owned()
}

fn default_minimum_copy_count() -> i64 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::fs;
    use std::io::Cursor;
    use std::time::UNIX_EPOCH;
    use super_duper_core::hasher::xxhash::hash_file_streaming;
    use super_duper_core::platform;
    use super_duper_core::storage::models::ScannedFile;
    use super_duper_core::telemetry::{
        StatusRunStart, StatusRunTerminal, TelemetryRunState, METRICS_CONTRACT_VERSION,
        PROGRESS_CONTRACT_VERSION,
    };
    use tempfile::TempDir;

    const HELLO: &str = r#"{"type":"request","id":"hello-1","method":"hello","params":{"protocolVersions":[1],"client":{"name":"protocol-test","version":"1.0.0"}}}"#;

    #[test]
    fn worker_status_database_defaults_beside_product_database_and_can_be_overridden() {
        let product = PathBuf::from("state").join("product.db");
        let options = WorkerOptions::new(product);
        assert_eq!(
            options.status_database_path,
            PathBuf::from("state/scan_status.db")
        );

        let overridden = options.with_status_database_path("diagnostics/custom-status.db");
        assert_eq!(
            overridden.status_database_path,
            PathBuf::from("diagnostics/custom-status.db")
        );
    }

    #[test]
    fn performance_queries_are_bounded_persisted_and_execution_disabled() {
        let temp = TempDir::new().unwrap();
        let product_path = temp.path().join("worker.db");
        let status_path = temp.path().join("status.db");
        let mut status = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
        for product_run_id in 1..=26 {
            let (run, _) = status
                .begin_run(&StatusRunStart {
                    operation_id: format!("performance-{product_run_id}"),
                    product_run_id: Some(product_run_id),
                    engine_version: "engine-test".to_owned(),
                    worker_version: Some("worker-test".to_owned()),
                    app_version: Some("app-test".to_owned()),
                    product_schema_version: Some(14),
                    input_signature: "same-input".to_owned(),
                    started_unix_millis: 1_700_000_000_000 + product_run_id,
                })
                .unwrap();
            status
                .finish_run(
                    run.id,
                    &StatusRunTerminal {
                        state: TelemetryRunState::Completed,
                        completed_unix_millis: 1_700_000_001_000 + product_run_id,
                        monotonic_nanos: 1_000_000_000,
                        error_code: None,
                        error_message: None,
                    },
                )
                .unwrap();
        }
        drop(status);

        let options = WorkerOptions::new(product_path).with_status_database_path(status_path);
        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(options.clone(), sender).unwrap();
        let mut session = WorkerSession::new(state);
        session.handle_line(HELLO).unwrap();
        let history: Value = serde_json::from_str(
            &session
                .handle_line(
                    &json!({
                        "type":"request", "id":"history", "method":"performance.run.page",
                        "params":{"pageSize":25}
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(history["result"]["runs"].as_array().unwrap().len(), 25);
        assert_eq!(history["result"]["runs"][0]["productRunId"], 26);
        assert!(history["result"]["nextBeforeId"].as_i64().is_some());
        assert_eq!(history["result"]["executorEnabled"], false);

        let snapshot: Value = serde_json::from_str(
            &session
                .handle_line(
                    &json!({
                        "type":"request", "id":"snapshot", "method":"performance.snapshot.get",
                        "params":{"productRunId":26}
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(snapshot["result"]["run"]["productRunId"], 26);
        assert_eq!(snapshot["result"]["phases"].as_array().unwrap().len(), 0);
        assert_eq!(snapshot["result"]["devices"].as_array().unwrap().len(), 0);
        assert_eq!(snapshot["result"]["executorEnabled"], false);
        drop(session);

        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(options, sender).unwrap();
        let mut restarted = WorkerSession::new(state);
        restarted.handle_line(HELLO).unwrap();
        let restored: Value = serde_json::from_str(
            &restarted
                .handle_line(
                    &json!({
                        "type":"request", "id":"restored", "method":"performance.snapshot.get",
                        "params":{"productRunId":26}
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(restored["result"]["run"]["state"], "completed");
    }

    fn execute(temp: &TempDir, requests: &[String]) -> Vec<Value> {
        let database = temp.path().join("worker.db");
        let input = Cursor::new(format!("{}\n", requests.join("\n")));
        let mut output = Vec::new();
        run_with_options(input, &mut output, WorkerOptions::new(database)).unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn response<'a>(frames: &'a [Value], id: &str) -> &'a Value {
        frames
            .iter()
            .find(|frame| frame["type"] == "response" && frame["id"] == id)
            .unwrap()
    }

    fn insert_recovery_review_protocol_fixture(temp: &TempDir) -> (i64, Vec<i64>, i64) {
        let database = temp.path().join("worker.db");
        let db = Database::open(database.to_str().unwrap()).unwrap();
        let session_id = db
            .create_session("Recovery protocol", &["Z:/persisted-only".to_owned()], &[])
            .unwrap();
        let parameters = RunParameters {
            roots: vec!["Z:/persisted-only".to_owned()],
            ignore_patterns: Vec::new(),
            directory_similarity_threshold_millis: 500,
            cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
            manual_location_exclusions: Vec::new(),
            registered_cloud_locations: Vec::new(),
            cloud_detection_status: CloudDetectionStatus::Complete,
        };
        let run_id = db
            .create_scan_run(session_id, &parameters, "protocol-test")
            .unwrap();
        let now = "2026-08-23T17:00:00Z";
        db.connection()
            .execute(
                "INSERT INTO review_plan (run_id, state, revision, created_at, updated_at)
                 VALUES (?1, 'active', 0, ?2, ?2)",
                params![run_id, now],
            )
            .unwrap();
        let plan_id = db.connection().last_insert_rowid();
        db.connection()
            .execute(
                "INSERT INTO preflight
                    (operation_id, run_id, plan_id, review_revision, snapshot_signature, status,
                     logical_removal_count, physical_removal_count, folder_removal_count,
                     affected_group_count, planned_removal_bytes, total_item_count,
                     processed_item_count, ready_count, changed_count, missing_count,
                     unavailable_count, conflict_count, created_at, completed_at)
                 VALUES ('protocol-preflight', ?1, ?2, 0, 'snapshot', 'completed',
                         3, 3, 0, 0, 3, 3, 3, 3, 0, 0, 0, 0, ?3, ?3)",
                params![run_id, plan_id, now],
            )
            .unwrap();
        let preflight_id = db.connection().last_insert_rowid();
        let mut preflight_item_ids = Vec::new();
        for ordinal in 0..3_i64 {
            db.connection()
                .execute(
                    "INSERT INTO preflight_item
                        (preflight_id, ordinal, target_kind, target_role, physical_key,
                         snapshot_path, outcome)
                     VALUES (?1, ?2, 'file', 'remove', ?3, ?4, 'ready')",
                    params![
                        preflight_id,
                        ordinal,
                        format!("protocol-key-{ordinal}"),
                        format!("Z:/never-inspected-{ordinal}.bin")
                    ],
                )
                .unwrap();
            preflight_item_ids.push(db.connection().last_insert_rowid());
        }
        db.connection()
            .execute(
                "INSERT INTO recycle_operation
                    (operation_id, run_id, plan_id, preflight_id, review_revision,
                     preflight_snapshot_signature, intent_signature, policy_version, status,
                     logical_removal_count, shell_item_count, physical_item_count,
                     folder_item_count, affected_group_count, planned_removal_bytes,
                     prepared_at, completed_at)
                 VALUES ('protocol-recovery', ?1, ?2, ?3, 0, 'snapshot', 'intent', 1,
                         'recovery_required', 3, 3, 3, 0, 0, 3, ?4, ?4)",
                params![run_id, plan_id, preflight_id, now],
            )
            .unwrap();
        let operation_id = db.connection().last_insert_rowid();
        db.connection()
            .execute(
                "INSERT INTO recycle_operation_batch
                    (recycle_operation_id, ordinal, item_signature, status, started_at)
                 VALUES (?1, 0, 'batch', 'ambiguous', ?2)",
                params![operation_id, now],
            )
            .unwrap();
        let batch_id = db.connection().last_insert_rowid();
        let mut unknown_ids = Vec::new();
        let mut non_unknown_id = 0;
        for (ordinal, preflight_item_id) in preflight_item_ids.into_iter().enumerate() {
            let result_status = if ordinal < 2 { "unknown" } else { "failed" };
            db.connection()
                .execute(
                    "INSERT INTO recycle_operation_item
                        (recycle_operation_id, batch_id, ordinal, preflight_item_id,
                         target_kind, physical_key, snapshot_path, planned_bytes,
                         eligibility_status, result_status, result_code, result_at)
                     VALUES (?1, ?2, ?3, ?4, 'file', ?5, ?6, 1, 'eligible', ?7,
                             'immutable-source', ?8)",
                    params![
                        operation_id,
                        batch_id,
                        ordinal as i64,
                        preflight_item_id,
                        format!("protocol-key-{ordinal}"),
                        format!("Z:/never-inspected-{ordinal}.bin"),
                        result_status,
                        now
                    ],
                )
                .unwrap();
            let item_id = db.connection().last_insert_rowid();
            if result_status == "unknown" {
                unknown_ids.push(item_id);
                db.connection()
                    .execute(
                        "INSERT INTO recycle_operation_recovery
                            (recycle_operation_id, batch_id, item_id, reason_code, created_at)
                         VALUES (?1, ?2, ?3, 'worker_interrupted_after_shell_start', ?4)",
                        params![operation_id, batch_id, item_id, now],
                    )
                    .unwrap();
            } else {
                non_unknown_id = item_id;
            }
        }
        (operation_id, unknown_ids, non_unknown_id)
    }

    #[test]
    fn recovery_review_protocol_is_bounded_idempotent_append_only_and_non_executing() {
        let temp = TempDir::new().unwrap();
        let (operation_id, unknown_ids, non_unknown_id) =
            insert_recovery_review_protocol_fixture(&temp);
        let frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                json!({"type":"request","id":"get","method":"recovery_review.get","params":{"recycleOperationId":operation_id}}).to_string(),
                json!({"type":"request","id":"first","method":"recovery_review.observation.record","params":{"requestId":"first","recycleOperationId":operation_id,"itemId":unknown_ids[0],"observation":"observed_in_recycle_bin","observedAt":"2026-08-23T17:01:00Z","note":"operator note","evidenceVersion":1}}).to_string(),
                json!({"type":"request","id":"replay","method":"recovery_review.observation.record","params":{"requestId":"first","recycleOperationId":operation_id,"itemId":unknown_ids[0],"observation":"observed_in_recycle_bin","observedAt":"2026-08-23T17:01:00Z","note":"operator note","evidenceVersion":1}}).to_string(),
                json!({"type":"request","id":"conflict","method":"recovery_review.observation.record","params":{"requestId":"first","recycleOperationId":operation_id,"itemId":unknown_ids[0],"observation":"observed_at_source","observedAt":"2026-08-23T17:01:00Z","note":"operator note","evidenceVersion":1}}).to_string(),
                json!({"type":"request","id":"invalid-kind","method":"recovery_review.observation.record","params":{"requestId":"invalid-kind","recycleOperationId":operation_id,"itemId":unknown_ids[1],"observation":"inferred_deleted","observedAt":"2026-08-23T17:02:00Z","evidenceVersion":1}}).to_string(),
                json!({"type":"request","id":"non-unknown","method":"recovery_review.observation.record","params":{"requestId":"non-unknown","recycleOperationId":operation_id,"itemId":non_unknown_id,"observation":"deferred_unresolved","observedAt":"2026-08-23T17:02:00Z","evidenceVersion":1}}).to_string(),
                json!({"type":"request","id":"second","method":"recovery_review.observation.record","params":{"requestId":"second","recycleOperationId":operation_id,"itemId":unknown_ids[1],"observation":"deferred_unresolved","observedAt":"2026-08-23T17:02:00Z","evidenceVersion":1}}).to_string(),
                json!({"type":"request","id":"page","method":"recovery_review.observation.page","params":{"recycleOperationId":operation_id,"pageSize":1,"currentOnly":true}}).to_string(),
                json!({"type":"request","id":"correction","method":"recovery_review.observation.record","params":{"requestId":"correction","recycleOperationId":operation_id,"itemId":unknown_ids[0],"observation":"observed_at_source","observedAt":"2026-08-23T17:03:00Z","evidenceVersion":1,"supersedesObservationId":1,"correctionReason":"corrected manual selection"}}).to_string(),
                json!({"type":"request","id":"history","method":"recovery_review.observation.page","params":{"recycleOperationId":operation_id,"pageSize":200,"currentOnly":false}}).to_string(),
                json!({"type":"request","id":"oversized-page","method":"recovery_review.observation.page","params":{"recycleOperationId":operation_id,"pageSize":201,"currentOnly":false}}).to_string(),
            ],
        );
        let next_cursor = response(&frames, "page")["result"]["nextCursor"]
            .as_str()
            .unwrap();
        let paging_frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                json!({"type":"request","id":"next-page","method":"recovery_review.observation.page","params":{"recycleOperationId":operation_id,"pageSize":1,"currentOnly":true,"cursor":next_cursor}}).to_string(),
                json!({"type":"request","id":"cursor-conflict","method":"recovery_review.observation.page","params":{"recycleOperationId":operation_id,"pageSize":1,"currentOnly":false,"cursor":next_cursor}}).to_string(),
            ],
        );
        assert_eq!(
            response(&frames, "get")["result"]["review"]["state"],
            "not_started"
        );
        assert_eq!(response(&frames, "get")["result"]["executorEnabled"], false);
        assert_eq!(
            response(&frames, "first")["result"]["review"]["state"],
            "in_progress"
        );
        assert_eq!(
            response(&frames, "first")["result"]["executorEnabled"],
            false
        );
        assert_eq!(response(&frames, "replay")["result"]["replayed"], true);
        assert_eq!(
            response(&frames, "conflict")["error"]["code"],
            "idempotency_conflict"
        );
        assert_eq!(
            response(&frames, "invalid-kind")["error"]["code"],
            "invalid_request"
        );
        assert_eq!(
            response(&frames, "non-unknown")["error"]["code"],
            "recovery_review_item_not_unknown"
        );
        assert_eq!(
            response(&frames, "second")["result"]["review"]["state"],
            "review_complete_with_unresolved_evidence"
        );
        assert_eq!(response(&frames, "page")["result"]["total"], 2);
        assert!(response(&frames, "page")["result"]["nextCursor"].is_string());
        assert_eq!(
            response(&paging_frames, "next-page")["result"]["observations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(response(&paging_frames, "next-page")["result"]["nextCursor"].is_null());
        assert_eq!(
            response(&paging_frames, "cursor-conflict")["error"]["code"],
            "invalid_cursor"
        );
        assert_eq!(
            response(&frames, "correction")["result"]["observation"]["supersedesObservationId"],
            1
        );
        assert_eq!(response(&frames, "history")["result"]["total"], 3);
        assert_eq!(
            response(&frames, "history")["result"]["executorEnabled"],
            false
        );
        assert_eq!(
            response(&frames, "oversized-page")["error"]["code"],
            "invalid_request"
        );

        let db = Database::open(temp.path().join("worker.db").to_str().unwrap()).unwrap();
        let source: (String, String, i64, i64) = db
            .connection()
            .query_row(
                "SELECT operation.status, batch.status,
                        SUM(CASE WHEN item.result_status = 'unknown' THEN 1 ELSE 0 END),
                        COUNT(*)
                 FROM recycle_operation operation
                 JOIN recycle_operation_batch batch ON batch.recycle_operation_id = operation.id
                 JOIN recycle_operation_item item ON item.batch_id = batch.id
                 WHERE operation.id = ?1 GROUP BY operation.id, batch.id",
                params![operation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            source,
            ("recovery_required".to_owned(), "ambiguous".to_owned(), 2, 3)
        );
    }

    #[test]
    fn hello_and_handshake_requirement_are_correlated() {
        let temp = TempDir::new().unwrap();
        let frames = execute(
            &temp,
            &[
                r#"{"type":"request","id":"before","method":"app.status","params":{}}"#.to_owned(),
                HELLO.to_owned(),
                r#"{"type":"request","id":"status","method":"app.status","params":{}}"#.to_owned(),
            ],
        );
        assert_eq!(
            response(&frames, "before")["error"]["code"],
            "handshake_required"
        );
        assert_eq!(response(&frames, "hello-1")["result"]["protocolVersion"], 1);
        assert_eq!(response(&frames, "status")["ok"], true);
    }

    #[test]
    fn live_validation_protocol_is_bounded_idempotent_and_exposes_invalidated_working_state() {
        let temp = TempDir::new().unwrap();
        let database_path = temp.path().join("worker.db");
        let root = fs::canonicalize(temp.path()).unwrap();
        let paths = (0..3)
            .map(|index| root.join(format!("live-copy-{index}.bin")))
            .collect::<Vec<_>>();
        for path in &paths {
            fs::write(path, b"same live validation bytes").unwrap();
        }
        let db = Database::open(database_path.to_str().unwrap()).unwrap();
        let roots = vec![root.to_string_lossy().into_owned()];
        let session_id = db.create_session("Live validation", &roots, &[]).unwrap();
        let run_id = db
            .create_scan_run(
                session_id,
                &RunParameters {
                    roots: roots.clone(),
                    ignore_patterns: Vec::new(),
                    directory_similarity_threshold_millis: 500,
                    cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
                    manual_location_exclusions: Vec::new(),
                    registered_cloud_locations: Vec::new(),
                    cloud_detection_status: CloudDetectionStatus::Complete,
                },
                "test",
            )
            .unwrap();
        db.start_scan_run(run_id).unwrap();
        let snapshots = paths
            .iter()
            .map(|path| {
                let metadata = fs::metadata(path).unwrap();
                ScannedFile {
                    id: 0,
                    run_id,
                    root_path: roots[0].clone(),
                    canonical_path: path.to_string_lossy().into_owned(),
                    relative_path: path.file_name().unwrap().to_string_lossy().into_owned(),
                    file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
                    parent_dir: root.to_string_lossy().into_owned(),
                    drive_letter: String::new(),
                    file_size: metadata.len() as i64,
                    last_modified: metadata
                        .modified()
                        .unwrap()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                        .min(i64::MAX as u128) as i64,
                    partial_hash: None,
                    content_hash: Some(31),
                    file_identity: platform::file_identity(path).unwrap(),
                    warning_message: None,
                    marked_deleted: false,
                }
            })
            .collect::<Vec<_>>();
        db.insert_scanned_files(&snapshots).unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[(
                31,
                snapshots[0].file_size,
                snapshots
                    .iter()
                    .map(|file| file.canonical_path.clone())
                    .collect(),
            )],
        )
        .unwrap();
        db.complete_scan_run(
            run_id,
            3,
            snapshots[0].file_size * 3,
            3,
            1,
            0,
            snapshots[0].file_size * 2,
            0,
        )
        .unwrap();
        let group_id = db
            .connection()
            .query_row(
                "SELECT id FROM duplicate_group WHERE run_id = ?1",
                params![run_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        let ids = db
            .connection()
            .prepare("SELECT id FROM scanned_file WHERE run_id = ?1 ORDER BY canonical_path")
            .unwrap()
            .query_map(params![run_id], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        db.set_review_decision(
            "live-keep",
            run_id,
            group_id,
            ids[0],
            ReviewDecisionKind::Keep,
            0,
        )
        .unwrap();
        db.set_review_decision(
            "live-remove",
            run_id,
            group_id,
            ids[1],
            ReviewDecisionKind::Remove,
            1,
        )
        .unwrap();
        let immutable_before = db.connection().prepare(
            "SELECT canonical_path, file_size, last_modified FROM scanned_file WHERE run_id = ?1 ORDER BY id"
        ).unwrap().query_map(params![run_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)))
            .unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        drop(db);
        fs::remove_file(&paths[0]).unwrap();
        fs::write(
            &paths[1],
            b"modified by another process and now a different size",
        )
        .unwrap();
        let request = format!(
            r#"{{"type":"request","id":"validate","method":"review_live_validation.run","params":{{"operationId":"protocol-live","runId":{run_id},"groupId":{group_id},"expectedReviewRevision":2,"scope":"selection","fileIds":[{},{},{}]}}}}"#,
            ids[0], ids[1], ids[2]
        );
        let oversized = (1..=201)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                request.clone(),
                request.replace("\"id\":\"validate\"", "\"id\":\"replay\""),
                format!(
                    r#"{{"type":"request","id":"members-live","method":"duplicate_file_group.members","params":{{"runId":{run_id},"groupId":{group_id},"pageSize":200}}}}"#
                ),
                format!(
                    r#"{{"type":"request","id":"oversized","method":"review_live_validation.run","params":{{"operationId":"oversized","runId":{run_id},"groupId":{group_id},"expectedReviewRevision":2,"scope":"visible_page","fileIds":[{oversized}]}}}}"#
                ),
            ],
        );
        let validated = &response(&frames, "validate")["result"];
        assert_eq!(validated["summary"]["itemCount"], 3);
        assert_eq!(validated["summary"]["missingCount"], 1);
        assert_eq!(validated["summary"]["changedCount"], 1);
        assert_eq!(validated["summary"]["invalidatedDecisionCount"], 2);
        assert_eq!(response(&frames, "replay")["result"]["replayed"], true);
        let members = response(&frames, "members-live")["result"]["members"]
            .as_array()
            .unwrap();
        assert_eq!(members[0]["decision"], "undecided");
        assert_eq!(members[0]["invalidatedDecision"], "keep");
        assert_eq!(members[1]["decision"], "undecided");
        assert_eq!(members[1]["invalidatedDecision"], "remove");
        assert_eq!(
            response(&frames, "oversized")["error"]["code"],
            "invalid_request"
        );
        let reopened = Database::open(database_path.to_str().unwrap()).unwrap();
        let immutable_after = reopened.connection().prepare(
            "SELECT canonical_path, file_size, last_modified FROM scanned_file WHERE run_id = ?1 ORDER BY id"
        ).unwrap().query_map(params![run_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)))
            .unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(immutable_after, immutable_before);
    }

    #[test]
    fn watcher_overflow_protocol_is_durable_visible_bounded_and_generation_bound() {
        let temp = TempDir::new().unwrap();
        let database_path = temp.path().join("worker.db");
        let root = fs::canonicalize(temp.path()).unwrap();
        let root_path = root.to_string_lossy().into_owned();
        let paths = (0..3)
            .map(|index| root.join(format!("overflow-copy-{index}.bin")))
            .collect::<Vec<_>>();
        for path in &paths {
            fs::write(path, b"same overflow bytes").unwrap();
        }
        let db = Database::open(database_path.to_str().unwrap()).unwrap();
        let session_id = db
            .create_session("Overflow protocol", std::slice::from_ref(&root_path), &[])
            .unwrap();
        let run_id = db
            .create_scan_run(
                session_id,
                &RunParameters {
                    roots: vec![root_path.clone()],
                    ignore_patterns: Vec::new(),
                    directory_similarity_threshold_millis: 500,
                    cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
                    manual_location_exclusions: Vec::new(),
                    registered_cloud_locations: Vec::new(),
                    cloud_detection_status: CloudDetectionStatus::Complete,
                },
                "test",
            )
            .unwrap();
        db.start_scan_run(run_id).unwrap();
        let snapshots = paths
            .iter()
            .map(|path| {
                let metadata = fs::metadata(path).unwrap();
                ScannedFile {
                    id: 0,
                    run_id,
                    root_path: root_path.clone(),
                    canonical_path: path.to_string_lossy().into_owned(),
                    relative_path: path.file_name().unwrap().to_string_lossy().into_owned(),
                    file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
                    parent_dir: root_path.clone(),
                    drive_letter: String::new(),
                    file_size: metadata.len() as i64,
                    last_modified: metadata
                        .modified()
                        .unwrap()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                        .min(i64::MAX as u128) as i64,
                    partial_hash: None,
                    content_hash: Some(44),
                    file_identity: platform::file_identity(path).unwrap(),
                    warning_message: None,
                    marked_deleted: false,
                }
            })
            .collect::<Vec<_>>();
        db.insert_scanned_files(&snapshots).unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[(
                44,
                snapshots[0].file_size,
                snapshots
                    .iter()
                    .map(|file| file.canonical_path.clone())
                    .collect(),
            )],
        )
        .unwrap();
        db.complete_scan_run(
            run_id,
            3,
            snapshots[0].file_size * 3,
            3,
            1,
            0,
            snapshots[0].file_size * 2,
            0,
        )
        .unwrap();
        let immutable_before = db
            .connection()
            .prepare(
                "SELECT canonical_path, file_size, last_modified
                 FROM scanned_file WHERE run_id = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map(params![run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        drop(db);
        let encoded_root = serde_json::to_string(&root_path).unwrap();
        let encoded_paths = serde_json::to_string(
            &paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let hint_frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                format!(
                    r#"{{"type":"request","id":"hint-burst","method":"review_live_hint.batch","params":{{"runId":{run_id},"rootPath":{encoded_root},"eventCount":1000,"paths":{encoded_paths}}}}}"#
                ),
            ],
        );
        let hint_result = &response(&hint_frames, "hint-burst")["result"];
        assert_eq!(hint_result["eventCount"], 1000);
        assert_eq!(hint_result["coalescedPathCount"], 3);
        assert_eq!(hint_result["items"].as_array().unwrap().len(), 3);
        assert_eq!(hint_result["executorEnabled"], false);
        let hint_events = hint_frames
            .iter()
            .filter(|frame| frame["event"] == "result.state_changed")
            .collect::<Vec<_>>();
        assert_eq!(hint_events.len(), 1);
        assert_eq!(hint_events[0]["data"]["eventCount"], 1000);
        assert_eq!(hint_events[0]["data"]["items"].as_array().unwrap().len(), 3);
        let overflow_request = format!(
            r#"{{"type":"request","id":"overflow","method":"review_live_root.overflow","params":{{"operationId":"watcher-overflow-1","runId":{run_id},"rootPath":{encoded_root}}}}}"#
        );
        let first_frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                overflow_request.clone(),
                overflow_request.replace("\"id\":\"overflow\"", "\"id\":\"overflow-replay\""),
                format!(
                    r#"{{"type":"request","id":"dirty-list","method":"review_live_root.list","params":{{"runId":{run_id}}}}}"#
                ),
                format!(
                    r#"{{"type":"request","id":"reconcile-first","method":"review_live_root.reconcile","params":{{"operationId":"reconcile-root-1","runId":{run_id},"rootPath":{encoded_root},"expectedDirtyRevision":1,"expectedReviewRevision":0,"pageSize":2}}}}"#
                ),
            ],
        );
        assert_eq!(
            response(&first_frames, "overflow")["result"]["root"]["state"],
            "dirty"
        );
        assert_eq!(
            response(&first_frames, "overflow")["result"]["executorEnabled"],
            false
        );
        assert_eq!(
            response(&first_frames, "overflow-replay")["result"]["replayed"],
            true
        );
        assert_eq!(response(&first_frames, "dirty-list")["result"]["total"], 1);
        let first = &response(&first_frames, "reconcile-first")["result"];
        assert_eq!(first["summary"]["itemCount"], 2);
        assert_eq!(first["root"]["reconciliationRequired"], true);
        assert_eq!(first["root"]["reconciledItemCount"], 2);
        assert_eq!(first["executorEnabled"], false);

        let second_frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                format!(
                    r#"{{"type":"request","id":"restart-list","method":"review_live_root.list","params":{{"runId":{run_id}}}}}"#
                ),
                format!(
                    r#"{{"type":"request","id":"reconcile-second","method":"review_live_root.reconcile","params":{{"operationId":"reconcile-root-2","runId":{run_id},"rootPath":{encoded_root},"expectedDirtyRevision":1,"expectedReviewRevision":0,"pageSize":2}}}}"#
                ),
                format!(
                    r#"{{"type":"request","id":"clean-list","method":"review_live_root.list","params":{{"runId":{run_id}}}}}"#
                ),
                format!(
                    r#"{{"type":"request","id":"oversized-reconcile","method":"review_live_root.reconcile","params":{{"operationId":"reconcile-root-oversized","runId":{run_id},"rootPath":{encoded_root},"expectedDirtyRevision":1,"expectedReviewRevision":0,"pageSize":201}}}}"#
                ),
                format!(
                    r#"{{"type":"request","id":"overflow-2","method":"review_live_root.overflow","params":{{"operationId":"watcher-overflow-2","runId":{run_id},"rootPath":{encoded_root}}}}}"#
                ),
                format!(
                    r#"{{"type":"request","id":"stale-reconcile","method":"review_live_root.reconcile","params":{{"operationId":"stale-reconcile","runId":{run_id},"rootPath":{encoded_root},"expectedDirtyRevision":1,"expectedReviewRevision":0,"pageSize":2}}}}"#
                ),
            ],
        );
        assert_eq!(
            response(&second_frames, "restart-list")["result"]["total"],
            1
        );
        assert_eq!(
            response(&second_frames, "restart-list")["result"]["roots"][0]["reconciledItemCount"],
            2
        );
        let second = &response(&second_frames, "reconcile-second")["result"];
        assert_eq!(second["summary"]["itemCount"], 1);
        assert_eq!(second["root"]["reconciliationRequired"], false);
        assert_eq!(second["root"]["reconciledItemCount"], 3);
        assert_eq!(response(&second_frames, "clean-list")["result"]["total"], 0);
        assert_eq!(
            response(&second_frames, "oversized-reconcile")["error"]["code"],
            "invalid_request"
        );
        assert_eq!(
            response(&second_frames, "overflow-2")["result"]["root"]["dirtyRevision"],
            2
        );
        assert_eq!(
            response(&second_frames, "stale-reconcile")["error"]["code"],
            "dirty_generation_conflict"
        );

        let reopened = Database::open(database_path.to_str().unwrap()).unwrap();
        let immutable_after = reopened
            .connection()
            .prepare(
                "SELECT canonical_path, file_size, last_modified
                 FROM scanned_file WHERE run_id = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map(params![run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(immutable_after, immutable_before);
    }

    #[test]
    fn preflight_protocol_is_revision_bound_idempotent_paged_and_non_deleting() {
        let temp = TempDir::new().unwrap();
        let database_path = temp.path().join("worker.db");
        let root = fs::canonicalize(temp.path()).unwrap();
        let remove_path = root.join("remove.bin");
        let survivor_path = root.join("survivor.bin");
        fs::write(&remove_path, b"worker preflight fixture").unwrap();
        fs::write(&survivor_path, b"worker preflight fixture").unwrap();
        let db = Database::open(database_path.to_str().unwrap()).unwrap();
        let roots = vec![root.to_string_lossy().into_owned()];
        let session_id = db.create_session("Preflight", &roots, &[]).unwrap();
        let parameters = RunParameters {
            roots: roots.clone(),
            ignore_patterns: Vec::new(),
            directory_similarity_threshold_millis: 500,
            cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
            manual_location_exclusions: Vec::new(),
            registered_cloud_locations: Vec::new(),
            cloud_detection_status: CloudDetectionStatus::Complete,
        };
        let run_id = db.create_scan_run(session_id, &parameters, "test").unwrap();
        db.start_scan_run(run_id).unwrap();
        let snapshots = [&remove_path, &survivor_path]
            .into_iter()
            .map(|path| {
                let canonical = fs::canonicalize(path).unwrap();
                let metadata = fs::metadata(&canonical).unwrap();
                ScannedFile {
                    id: 0,
                    run_id,
                    root_path: roots[0].clone(),
                    canonical_path: canonical.to_string_lossy().into_owned(),
                    relative_path: canonical
                        .strip_prefix(&root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    file_name: canonical
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    parent_dir: canonical.parent().unwrap().to_string_lossy().into_owned(),
                    drive_letter: String::new(),
                    file_size: metadata.len() as i64,
                    last_modified: metadata
                        .modified()
                        .unwrap()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                        .min(i64::MAX as u128) as i64,
                    partial_hash: None,
                    content_hash: Some(
                        hash_file_streaming(&canonical, &AtomicBool::new(false)).unwrap() as i64,
                    ),
                    file_identity: platform::file_identity(&canonical).unwrap(),
                    warning_message: None,
                    marked_deleted: false,
                }
            })
            .collect::<Vec<_>>();
        let hash = snapshots[0].content_hash.unwrap();
        let size = snapshots[0].file_size;
        let paths = snapshots
            .iter()
            .map(|file| file.canonical_path.clone())
            .collect::<Vec<_>>();
        db.insert_scanned_files(&snapshots).unwrap();
        db.insert_duplicate_groups(run_id, &[(hash, size, paths)])
            .unwrap();
        db.complete_scan_run(run_id, 2, size * 2, 2, 1, 0, size, 0)
            .unwrap();
        let group_id: i64 = db
            .connection()
            .query_row(
                "SELECT id FROM duplicate_group WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .unwrap();
        let ids = db
            .connection()
            .prepare("SELECT id FROM scanned_file WHERE run_id = ?1 ORDER BY canonical_path")
            .unwrap()
            .query_map(params![run_id], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        db.set_review_decision(
            "remove",
            run_id,
            group_id,
            ids[0],
            ReviewDecisionKind::Remove,
            0,
        )
        .unwrap();
        db.set_review_decision(
            "keep",
            run_id,
            group_id,
            ids[1],
            ReviewDecisionKind::Keep,
            1,
        )
        .unwrap();
        drop(db);

        let frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                r#"{"type":"request","id":"start","method":"preflight.start","params":{"operationId":"start-preflight","runId":1,"expectedReviewRevision":2}}"#.to_owned(),
                r#"{"type":"request","id":"replay","method":"preflight.start","params":{"operationId":"start-preflight","runId":1,"expectedReviewRevision":2}}"#.to_owned(),
                r#"{"type":"request","id":"latest","method":"preflight.get","params":{"runId":1}}"#.to_owned(),
                r#"{"type":"request","id":"page","method":"preflight.item.page","params":{"preflightId":1,"pageSize":1}}"#.to_owned(),
                r#"{"type":"request","id":"unknown","method":"preflight.get","params":{"runId":1,"unexpected":true}}"#.to_owned(),
            ],
        );
        assert_eq!(response(&frames, "start")["ok"], true);
        assert_eq!(response(&frames, "start")["result"]["replayed"], false);
        assert_eq!(
            response(&frames, "start")["result"]["preflight"]["reviewRevision"],
            2
        );
        assert_eq!(response(&frames, "replay")["result"]["replayed"], true);
        assert_eq!(response(&frames, "latest")["result"]["preflight"]["id"], 1);
        assert_eq!(
            response(&frames, "page")["result"]["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(response(&frames, "page")["result"]["nextCursor"].is_string());
        assert_eq!(
            response(&frames, "unknown")["error"]["code"],
            "invalid_request"
        );
        assert_eq!(fs::read(&remove_path).unwrap(), b"worker preflight fixture");
        assert_eq!(
            fs::read(&survivor_path).unwrap(),
            b"worker preflight fixture"
        );

        let operation_preflight_id = {
            let db = Database::open_connection(database_path.to_str().unwrap()).unwrap();
            let latest = db.latest_preflight_for_run(run_id).unwrap().unwrap();
            if latest.preflight.status == "completed" {
                latest.preflight.id
            } else {
                let fresh = db
                    .create_preflight("operation-fixture-preflight", run_id, 2)
                    .unwrap();
                db.validate_preflight(fresh.view.preflight.id, &AtomicBool::new(false), |_, _| {})
                    .unwrap()
                    .id
            }
        };

        let operation_frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                json!({"type":"request","id":"prepare-operation","method":"recycle_operation.prepare","params":{"operationId":"prepare-operation","runId":run_id,"preflightId":operation_preflight_id,"expectedReviewRevision":2}}).to_string(),
                json!({"type":"request","id":"replay-operation","method":"recycle_operation.prepare","params":{"operationId":"prepare-operation","runId":run_id,"preflightId":operation_preflight_id,"expectedReviewRevision":2}}).to_string(),
                r#"{"type":"request","id":"latest-operation","method":"recycle_operation.get","params":{"runId":1}}"#.to_owned(),
                r#"{"type":"request","id":"operation-page","method":"recycle_operation.item.page","params":{"recycleOperationId":1,"pageSize":1}}"#.to_owned(),
                r#"{"type":"request","id":"disabled-capability","method":"recycle_operation.eligibility.report","params":{"reportOperationId":"disabled-capability","recycleOperationId":1,"items":[{"itemId":1,"status":"non_recyclable","reasonCode":"executor_disabled"}]}}"#.to_owned(),
            ],
        );
        assert_eq!(
            response(&operation_frames, "prepare-operation")["ok"],
            true,
            "{:?}",
            response(&operation_frames, "prepare-operation")
        );
        assert_eq!(
            response(&operation_frames, "prepare-operation")["result"]["executorEnabled"],
            false
        );
        assert_eq!(
            response(&operation_frames, "prepare-operation")["result"]["operation"]["status"],
            "prepared"
        );
        assert_eq!(
            response(&operation_frames, "replay-operation")["result"]["replayed"],
            true
        );
        assert_eq!(
            response(&operation_frames, "latest-operation")["result"]["operation"]["preflightId"],
            operation_preflight_id
        );
        assert_eq!(
            response(&operation_frames, "operation-page")["result"]["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            response(&operation_frames, "disabled-capability")["result"]["operation"]["status"],
            "failed"
        );
        assert_eq!(fs::read(&remove_path).unwrap(), b"worker preflight fixture");
        assert_eq!(
            fs::read(&survivor_path).unwrap(),
            b"worker preflight fixture"
        );
    }

    #[test]
    fn preference_rules_persist_and_preview_with_revision_bound_cursors() {
        let temp = TempDir::new().unwrap();
        let database_path = temp.path().join("worker.db");
        let root_a = temp.path().join("preferred");
        let root_b = temp.path().join("backup");
        let db = Database::open(database_path.to_str().unwrap()).unwrap();
        let roots = vec![
            root_a.to_string_lossy().to_string(),
            root_b.to_string_lossy().to_string(),
        ];
        let session_id = db.create_session("Preview", &roots, &[]).unwrap();
        let run_parameters = RunParameters {
            roots: roots.clone(),
            ignore_patterns: Vec::new(),
            directory_similarity_threshold_millis: 500,
            cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
            manual_location_exclusions: Vec::new(),
            registered_cloud_locations: Vec::new(),
            cloud_detection_status: CloudDetectionStatus::Complete,
        };
        let run_id = db
            .create_scan_run(session_id, &run_parameters, "test")
            .unwrap();
        db.start_scan_run(run_id).unwrap();
        let mut files = Vec::new();
        for (index, hash) in [11_i64, 22].into_iter().enumerate() {
            for (copy, root) in roots.iter().enumerate() {
                let path = Path::new(root).join(format!("set-{index}-{copy}.bin"));
                files.push(ScannedFile {
                    id: 0,
                    run_id,
                    root_path: root.clone(),
                    canonical_path: path.to_string_lossy().to_string(),
                    relative_path: format!("set-{index}-{copy}.bin"),
                    file_name: format!("set-{index}-{copy}.bin"),
                    parent_dir: root.clone(),
                    drive_letter: String::new(),
                    file_size: 100,
                    last_modified: 1,
                    partial_hash: None,
                    content_hash: Some(hash),
                    file_identity: Some(format!("physical-{index}-{copy}")),
                    warning_message: None,
                    marked_deleted: false,
                });
            }
        }
        db.insert_scanned_files(&files).unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[
                (
                    11,
                    100,
                    files[..2]
                        .iter()
                        .map(|file| file.canonical_path.clone())
                        .collect(),
                ),
                (
                    22,
                    100,
                    files[2..]
                        .iter()
                        .map(|file| file.canonical_path.clone())
                        .collect(),
                ),
            ],
        )
        .unwrap();
        db.complete_scan_run(run_id, 4, 400, 4, 2, 0, 200, 0)
            .unwrap();
        drop(db);

        let first = execute(
            &temp,
            &[
                HELLO.to_owned(),
                json!({
                    "type":"request","id":"save","method":"preference_rule.save",
                    "params":{
                        "operationId":"save-rule","name":"Preferred locations",
                        "roots":roots,"expectedRevision":0
                    }
                }).to_string(),
                json!({
                    "type":"request","id":"preview","method":"preference_rule.preview",
                    "params":{
                        "runId":run_id,"ruleId":1,"ruleRevision":1,"reviewRevision":0,
                        "pageSize":1,"scope":{"kind":"completed_run"}
                    }
                }).to_string(),
                json!({
                    "type":"request","id":"invalid-selected-scope","method":"preference_rule.preview",
                    "params":{
                        "runId":run_id,"ruleId":1,"ruleRevision":1,"reviewRevision":0,
                        "scope":{"kind":"selected_sets","groupIds":[1],"filter":{"minimumCopyCount":3}}
                    }
                }).to_string(),
                r#"{"type":"request","id":"list-rules","method":"preference_rule.list","params":{}}"#.to_owned(),
                r#"{"type":"request","id":"get-rule","method":"preference_rule.get","params":{"ruleId":1}}"#.to_owned(),
            ],
        );
        assert_eq!(response(&first, "save")["result"]["rule"]["revision"], 1);
        assert_eq!(response(&first, "list-rules")["result"]["total"], 1);
        assert_eq!(
            response(&first, "get-rule")["result"]["rule"]["roots"][0],
            roots[0]
        );
        let preview = &response(&first, "preview")["result"];
        assert_eq!(preview["total"], 2);
        assert_eq!(preview["groups"].as_array().unwrap().len(), 1);
        assert_eq!(preview["summary"]["affectedGroupCount"], 2);
        assert_eq!(preview["summary"]["proposedRemovePhysicalItemCount"], 2);
        assert_eq!(
            response(&first, "invalid-selected-scope")["error"]["code"],
            "invalid_scope"
        );
        let cursor = preview["nextCursor"].as_str().unwrap();
        let preview_signature = preview["previewSignature"].as_str().unwrap();

        let application_frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                json!({
                    "type":"request","id":"apply","method":"preference_rule.apply",
                    "params":{
                        "operationId":"apply-rule","runId":run_id,"ruleId":1,
                        "ruleRevision":1,"sourceReviewRevision":0,
                        "previewSignature":preview_signature,
                        "scope":{"kind":"completed_run"}
                    }
                }).to_string(),
                json!({
                    "type":"request","id":"applications","method":"preference_rule.application.page",
                    "params":{"runId":run_id,"ruleId":1,"state":"active","pageSize":20}
                }).to_string(),
                json!({
                    "type":"request","id":"application-detail","method":"preference_rule.application.get",
                    "params":{"runId":run_id,"applicationId":1}
                }).to_string(),
                json!({
                    "type":"request","id":"members","method":"duplicate_file_group.members",
                    "params":{"runId":run_id,"groupId":1,"pageSize":20}
                }).to_string(),
                json!({
                    "type":"request","id":"reverse","method":"preference_rule.application.reverse",
                    "params":{
                        "operationId":"reverse-rule","runId":run_id,
                        "applicationId":1,"expectedRevision":1
                    }
                }).to_string(),
                json!({
                    "type":"request","id":"plan-after-reverse","method":"review_plan.get",
                    "params":{"runId":run_id}
                }).to_string(),
            ],
        );
        assert_eq!(
            response(&application_frames, "apply")["result"]["application"]["appliedRevision"],
            1
        );
        assert_eq!(
            response(&application_frames, "apply")["result"]["application"]["summary"]
                ["ruleRemovePathCount"],
            2
        );
        assert_eq!(
            response(&application_frames, "applications")["result"]["total"],
            1
        );
        assert!(
            response(&application_frames, "applications")["result"]["applications"][0]
                .get("ruleRoots")
                .is_none()
        );
        assert_eq!(
            response(&application_frames, "application-detail")["result"]["application"]
                ["ruleRoots"][0],
            roots[0]
        );
        assert_eq!(
            response(&application_frames, "members")["result"]["members"][0]["decisionProvenance"],
            "rule"
        );
        assert_eq!(
            response(&application_frames, "members")["result"]["members"][0]
                ["decisionApplicationId"],
            1
        );
        assert_eq!(
            response(&application_frames, "reverse")["result"]["appliedRevision"],
            2
        );
        assert_eq!(
            response(&application_frames, "plan-after-reverse")["result"]["plan"]["revision"],
            2
        );

        let second = execute(
            &temp,
            &[
                HELLO.to_owned(),
                json!({
                    "type":"request","id":"update","method":"preference_rule.save",
                    "params":{
                        "operationId":"update-rule","ruleId":1,"name":"Preferred locations",
                        "roots":[roots[1],roots[0]],"expectedRevision":1
                    }
                })
                .to_string(),
                json!({
                    "type":"request","id":"stale-cursor","method":"preference_rule.preview",
                    "params":{
                        "runId":run_id,"ruleId":1,"ruleRevision":2,"reviewRevision":2,
                        "pageSize":1,"scope":{"kind":"completed_run"},"cursor":cursor
                    }
                })
                .to_string(),
            ],
        );
        assert_eq!(response(&second, "update")["result"]["rule"]["revision"], 2);
        assert_eq!(
            response(&second, "stale-cursor")["error"]["code"],
            "invalid_cursor"
        );
    }

    #[test]
    fn session_crud_uses_typed_contracts_and_case_insensitive_names() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        fs::create_dir(&root).unwrap();
        let root = root.to_string_lossy().replace('\\', "\\\\");
        let frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                format!(r#"{{"type":"request","id":"create","method":"session.create","params":{{"name":"Photos","roots":["{root}"],"ignorePatterns":["**/*.tmp"]}}}}"#),
                format!(r#"{{"type":"request","id":"conflict","method":"session.create","params":{{"name":"photos","roots":["{root}"],"ignorePatterns":[]}}}}"#),
                format!(r#"{{"type":"request","id":"update","method":"session.update","params":{{"sessionId":1,"name":"Pictures","roots":["{root}"],"ignorePatterns":[]}}}}"#),
                r#"{"type":"request","id":"get","method":"session.get","params":{"sessionId":1}}"#.to_owned(),
                r#"{"type":"request","id":"list","method":"session.list","params":{}}"#.to_owned(),
                r#"{"type":"request","id":"delete","method":"session.delete","params":{"sessionId":1}}"#.to_owned(),
            ],
        );
        assert_eq!(
            response(&frames, "create")["result"]["session"]["name"],
            "Photos"
        );
        assert_eq!(
            response(&frames, "conflict")["error"]["code"],
            "session_name_conflict"
        );
        assert_eq!(
            response(&frames, "get")["result"]["session"]["name"],
            "Pictures"
        );
        assert_eq!(response(&frames, "list")["result"]["total"], 1);
        assert_eq!(response(&frames, "delete")["ok"], true);
    }

    #[test]
    fn extension_match_mode_is_bound_to_all_duplicate_file_cursor_signatures() {
        let group_any = group_query_signature(
            1,
            DuplicateFileGroupSortField::RecoverableBytes,
            SortDirection::Descending,
            None,
            DuplicateFilePathMatchMode::Substring,
            Some("bin"),
            DuplicateFileExtensionMatchMode::AnyMember,
            0,
            2,
            false,
            None,
            None,
        );
        let group_all = group_query_signature(
            1,
            DuplicateFileGroupSortField::RecoverableBytes,
            SortDirection::Descending,
            None,
            DuplicateFilePathMatchMode::Substring,
            Some("bin"),
            DuplicateFileExtensionMatchMode::AllMembers,
            0,
            2,
            false,
            None,
            None,
        );
        let group_cursor = encode_group_cursor(
            &DuplicateFileGroupResult {
                id: 1,
                run_id: 1,
                file_size: 100,
                file_count: 2,
                recoverable_bytes: 100,
                representative_name: "one.bin".to_owned(),
                distinct_selected_root_count: 1,
                distinct_drive_count: 1,
            },
            DuplicateFileGroupSortField::RecoverableBytes,
            false,
            &group_any,
        )
        .unwrap();
        assert_eq!(
            decode_cursor(Some(&group_cursor), "duplicate-file-groups", &group_all)
                .unwrap_err()
                .code,
            "invalid_cursor"
        );

        let root_any = selected_root_facet_query_signature(
            1,
            DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            SortDirection::Descending,
            None,
            DuplicateFilePathMatchMode::Substring,
            Some("bin"),
            DuplicateFileExtensionMatchMode::AnyMember,
            0,
            2,
            false,
            None,
        );
        let root_all = selected_root_facet_query_signature(
            1,
            DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            SortDirection::Descending,
            None,
            DuplicateFilePathMatchMode::Substring,
            Some("bin"),
            DuplicateFileExtensionMatchMode::AllMembers,
            0,
            2,
            false,
            None,
        );
        let root_cursor = encode_selected_root_facet_cursor(
            &DuplicateFileSelectedRootFacetResult {
                cursor_id: 1,
                value: "/root".to_owned(),
                matching_group_count: 1,
            },
            DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            false,
            &root_any,
        )
        .unwrap();
        assert_eq!(
            decode_cursor(
                Some(&root_cursor),
                "duplicate-file-selected-root-facets",
                &root_all,
            )
            .unwrap_err()
            .code,
            "invalid_cursor"
        );

        let drive_any = drive_facet_query_signature(
            1,
            DuplicateFileDriveFacetSortField::MatchingGroupCount,
            SortDirection::Descending,
            None,
            DuplicateFilePathMatchMode::Substring,
            Some("bin"),
            DuplicateFileExtensionMatchMode::AnyMember,
            0,
            2,
            false,
            None,
        );
        let drive_all = drive_facet_query_signature(
            1,
            DuplicateFileDriveFacetSortField::MatchingGroupCount,
            SortDirection::Descending,
            None,
            DuplicateFilePathMatchMode::Substring,
            Some("bin"),
            DuplicateFileExtensionMatchMode::AllMembers,
            0,
            2,
            false,
            None,
        );
        let drive_cursor = encode_drive_facet_cursor(
            &DuplicateFileDriveFacetResult {
                cursor_id: 1,
                value: "D:".to_owned(),
                matching_group_count: 1,
            },
            DuplicateFileDriveFacetSortField::MatchingGroupCount,
            false,
            &drive_any,
        )
        .unwrap();
        assert_eq!(
            decode_cursor(
                Some(&drive_cursor),
                "duplicate-file-drive-facets",
                &drive_all,
            )
            .unwrap_err()
            .code,
            "invalid_cursor"
        );
    }

    #[test]
    fn duplicate_file_pages_use_opaque_query_bound_cursors_and_run_owned_members() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        let session_id = db
            .create_session("Results", &["/root".to_owned()], &[])
            .unwrap();
        let run_id = db
            .create_scan_run(
                session_id,
                &RunParameters {
                    roots: vec!["/root".to_owned()],
                    ignore_patterns: vec![],
                    directory_similarity_threshold_millis: 500,
                    cloud_policy: Default::default(),
                    manual_location_exclusions: vec![],
                    registered_cloud_locations: vec![],
                    cloud_detection_status: Default::default(),
                },
                "test",
            )
            .unwrap();
        db.start_scan_run(run_id).unwrap();
        db.insert_scanned_files(&[
            super_duper_core::storage::models::ScannedFile {
                id: 0,
                run_id,
                root_path: "/root".to_owned(),
                canonical_path: "/root/a".to_owned(),
                relative_path: "a".to_owned(),
                file_name: "a".to_owned(),
                parent_dir: "/root".to_owned(),
                drive_letter: String::new(),
                file_size: 100,
                last_modified: 1_700_000_000_000_000_000,
                partial_hash: None,
                content_hash: Some(11),
                file_identity: None,
                warning_message: None,
                marked_deleted: false,
            },
            super_duper_core::storage::models::ScannedFile {
                id: 0,
                run_id,
                root_path: "/root".to_owned(),
                canonical_path: "/root/a-copy.JPG".to_owned(),
                relative_path: "a-copy.JPG".to_owned(),
                file_name: "a-copy.JPG".to_owned(),
                parent_dir: "/root".to_owned(),
                drive_letter: String::new(),
                file_size: 100,
                last_modified: 1_700_000_000_000_000_001,
                partial_hash: None,
                content_hash: Some(11),
                file_identity: None,
                warning_message: None,
                marked_deleted: false,
            },
            super_duper_core::storage::models::ScannedFile {
                id: 0,
                run_id,
                root_path: "/root".to_owned(),
                canonical_path: "/root/b.bin".to_owned(),
                relative_path: "b.bin".to_owned(),
                file_name: "b.bin".to_owned(),
                parent_dir: "/root".to_owned(),
                drive_letter: "D:".to_owned(),
                file_size: 200,
                last_modified: 1_700_000_000_000_000_002,
                partial_hash: None,
                content_hash: Some(22),
                file_identity: None,
                warning_message: None,
                marked_deleted: false,
            },
            super_duper_core::storage::models::ScannedFile {
                id: 0,
                run_id,
                root_path: "/archive".to_owned(),
                canonical_path: "/root/b-copy.bin".to_owned(),
                relative_path: "copies/b-copy.bin".to_owned(),
                file_name: "b-copy.bin".to_owned(),
                parent_dir: "/root".to_owned(),
                drive_letter: "E:".to_owned(),
                file_size: 200,
                last_modified: 1_700_000_000_000_000_003,
                partial_hash: None,
                content_hash: Some(22),
                file_identity: None,
                warning_message: None,
                marked_deleted: false,
            },
            super_duper_core::storage::models::ScannedFile {
                id: 0,
                run_id,
                root_path: "/root".to_owned(),
                canonical_path: "/root/b-third.bin".to_owned(),
                relative_path: "b-third.bin".to_owned(),
                file_name: "b-third.bin".to_owned(),
                parent_dir: "/root".to_owned(),
                drive_letter: "D:".to_owned(),
                file_size: 200,
                last_modified: 1_700_000_000_000_000_004,
                partial_hash: None,
                content_hash: Some(22),
                file_identity: None,
                warning_message: None,
                marked_deleted: false,
            },
        ])
        .unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[
                (
                    11,
                    100,
                    vec!["/root/a".to_owned(), "/root/a-copy.JPG".to_owned()],
                ),
                (
                    22,
                    200,
                    vec![
                        "/root/b.bin".to_owned(),
                        "/root/b-copy.bin".to_owned(),
                        "/root/b-third.bin".to_owned(),
                    ],
                ),
            ],
        )
        .unwrap();
        db.complete_scan_run(run_id, 5, 800, 5, 2, 0, 500, 0)
            .unwrap();
        drop(db);

        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let mut worker = WorkerSession::new(state);
        assert_eq!(
            serde_json::from_str::<Value>(&worker.handle_line(HELLO).unwrap()).unwrap()["ok"],
            true
        );
        let first: Value = serde_json::from_str(
            &worker
                .handle_line(
                    r#"{"type":"request","id":"first","method":"duplicate_file_group.page","params":{"runId":1,"pageSize":1,"sort":{"field":"recoverableBytes","direction":"descending"},"filter":{"search":"","minimumSize":"0"}}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(first["result"]["total"], 2);
        assert_eq!(first["result"]["summary"]["matchingGroupCount"], 2);
        assert_eq!(first["result"]["summary"]["matchingCopyCount"], 5);
        assert_eq!(
            first["result"]["summary"]["potentialRecoverableBytes"],
            "500"
        );
        assert_eq!(first["result"]["summary"]["largestRecoverableBytes"], "400");
        assert_eq!(first["result"]["summary"]["distinctSelectedRootCount"], 2);
        assert_eq!(first["result"]["summary"]["distinctDriveCount"], 2);
        assert_eq!(first["result"]["summary"]["acrossDriveGroupCount"], 1);
        assert_eq!(first["result"]["groups"][0]["groupSize"], "200");
        assert_eq!(first["result"]["groups"][0]["distinctSelectedRootCount"], 2);
        assert_eq!(first["result"]["groups"][0]["distinctDriveCount"], 2);
        let across_drives_request = json!({
            "type":"request",
            "id":"across-drives",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":true}
            }
        });
        let across_drives: Value = serde_json::from_str(
            &worker
                .handle_line(&across_drives_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(across_drives["result"]["total"], 1);
        assert_eq!(across_drives["result"]["summary"]["matchingGroupCount"], 1);
        assert_eq!(
            across_drives["result"]["summary"]["acrossDriveGroupCount"],
            1
        );
        assert_eq!(
            across_drives["result"]["groups"][0]["distinctDriveCount"],
            2
        );
        let minimum_copy_count_request = json!({
            "type":"request",
            "id":"minimum-copy-count",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"search":"","minimumSize":"0","minimumCopyCount":3}
            }
        });
        let minimum_copy_count: Value = serde_json::from_str(
            &worker
                .handle_line(&minimum_copy_count_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(minimum_copy_count["result"]["total"], 1);
        assert_eq!(
            minimum_copy_count["result"]["summary"]["matchingCopyCount"],
            3
        );
        assert_eq!(minimum_copy_count["result"]["groups"][0]["copyCount"], 3);
        let minimum_size_request = json!({
            "type":"request",
            "id":"minimum-size",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"minimumSize":"200"}
            }
        });
        let minimum_size: Value = serde_json::from_str(
            &worker
                .handle_line(&minimum_size_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(minimum_size["result"]["total"], 1);
        assert_eq!(minimum_size["result"]["summary"]["matchingGroupCount"], 1);
        assert_eq!(minimum_size["result"]["summary"]["matchingCopyCount"], 3);
        assert_eq!(minimum_size["result"]["groups"][0]["groupSize"], "200");
        let extension_request = json!({
            "type":"request",
            "id":"extension",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "filter":{"extension":"JPG"}
            }
        });
        let extension: Value =
            serde_json::from_str(&worker.handle_line(&extension_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(extension["result"]["total"], 1);
        assert_eq!(extension["result"]["summary"]["matchingGroupCount"], 1);
        assert_eq!(extension["result"]["summary"]["matchingCopyCount"], 2);
        assert_eq!(extension["result"]["groups"][0]["groupSize"], "100");
        let all_extension_request = json!({
            "type":"request",
            "id":"all-extension",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "filter":{"extension":"BIN","extensionMatch":"all"}
            }
        });
        let all_extension: Value = serde_json::from_str(
            &worker
                .handle_line(&all_extension_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(all_extension["result"]["total"], 1);
        assert_eq!(all_extension["result"]["summary"]["matchingGroupCount"], 1);
        assert_eq!(all_extension["result"]["summary"]["matchingCopyCount"], 3);
        assert_eq!(all_extension["result"]["groups"][0]["groupSize"], "200");
        let no_extension_request = json!({
            "type":"request",
            "id":"no-extension",
            "method":"duplicate_file_group.page",
            "params":{"runId":1,"filter":{"extension":""}}
        });
        let no_extension: Value = serde_json::from_str(
            &worker
                .handle_line(&no_extension_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(no_extension["result"]["total"], 1);
        assert_eq!(no_extension["result"]["groups"][0]["groupSize"], "100");
        let all_no_extension_request = json!({
            "type":"request",
            "id":"all-no-extension",
            "method":"duplicate_file_group.page",
            "params":{"runId":1,"filter":{"extension":"","extensionMatch":"all"}}
        });
        let all_no_extension: Value = serde_json::from_str(
            &worker
                .handle_line(&all_no_extension_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(all_no_extension["result"]["total"], 0);
        let invalid_extension_request = json!({
            "type":"request",
            "id":"invalid-extension",
            "method":"duplicate_file_group.page",
            "params":{"runId":1,"filter":{"extension":"tar.gz"}}
        });
        let invalid_extension: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_extension_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_extension["error"]["code"], "invalid_request");
        assert_eq!(
            invalid_extension["error"]["details"]["field"],
            "filter.extension"
        );
        let invalid_extension_match_request = json!({
            "type":"request",
            "id":"invalid-extension-match",
            "method":"duplicate_file_group.page",
            "params":{"runId":1,"filter":{"extension":"bin","extensionMatch":"representative"}}
        });
        let invalid_extension_match: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_extension_match_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_extension_match["error"]["code"], "invalid_request");
        assert_eq!(
            invalid_extension_match["error"]["details"]["field"],
            "filter.extensionMatch"
        );
        let invalid_minimum_copy_count_request = json!({
            "type":"request",
            "id":"invalid-minimum-copy-count",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "filter":{"minimumCopyCount":1}
            }
        });
        let invalid_minimum_copy_count: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_minimum_copy_count_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            invalid_minimum_copy_count["error"]["code"],
            "invalid_request"
        );
        assert_eq!(
            invalid_minimum_copy_count["error"]["details"]["field"],
            "filter.minimumCopyCount"
        );
        let root_facet_request = json!({
            "type":"request",
            "id":"root-facet",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":false}
            }
        });
        let root_facet: Value =
            serde_json::from_str(&worker.handle_line(&root_facet_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(root_facet["result"]["total"], 2);
        assert_eq!(root_facet["result"]["facets"][0]["value"], "/root");
        assert_eq!(root_facet["result"]["facets"][0]["matchingGroupCount"], 2);
        let root_facet_cursor = root_facet["result"]["nextCursor"].as_str().unwrap();
        let next_root_facet_request = json!({
            "type":"request",
            "id":"root-facet-next",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":false},
                "cursor":root_facet_cursor
            }
        });
        let next_root_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&next_root_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(next_root_facet["result"]["facets"][0]["value"], "/archive");
        assert!(next_root_facet["result"]["previousCursor"].is_string());
        let invalid_root_facet_request = json!({
            "type":"request",
            "id":"root-facet-invalid",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":false},
                "cursor":root_facet_cursor
            }
        });
        let invalid_root_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_root_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_root_facet["error"]["code"], "invalid_cursor");
        let invalid_root_facet_copy_count_request = json!({
            "type":"request",
            "id":"root-facet-copy-count-invalid",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{"minimumCopyCount":3},
                "cursor":root_facet_cursor
            }
        });
        let invalid_root_facet_copy_count: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_root_facet_copy_count_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            invalid_root_facet_copy_count["error"]["code"],
            "invalid_cursor"
        );
        let invalid_root_facet_size_request = json!({
            "type":"request",
            "id":"root-facet-size-invalid",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{"minimumSize":"200"},
                "cursor":root_facet_cursor
            }
        });
        let invalid_root_facet_size: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_root_facet_size_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_root_facet_size["error"]["code"], "invalid_cursor");
        let named_root_facet_request = json!({
            "type":"request",
            "id":"root-facet-name",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":false}
            }
        });
        let named_root_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&named_root_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(named_root_facet["result"]["facets"][0]["value"], "/archive");
        let minimum_copy_count_root_facet_request = json!({
            "type":"request",
            "id":"root-facet-minimum-copy-count",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{"minimumCopyCount":3}
            }
        });
        let minimum_copy_count_root_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&minimum_copy_count_root_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(minimum_copy_count_root_facet["result"]["total"], 2);
        assert!(minimum_copy_count_root_facet["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let extension_root_facet_request = json!({
            "type":"request",
            "id":"root-facet-extension",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{"runId":1,"pageSize":25,"filter":{"extension":"BIN"}}
        });
        let extension_root_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&extension_root_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(extension_root_facet["result"]["total"], 2);
        assert!(extension_root_facet["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let all_extension_root_facet_request = json!({
            "type":"request",
            "id":"root-facet-all-extension",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{"runId":1,"pageSize":25,"filter":{"extension":"BIN","extensionMatch":"all"}}
        });
        let all_extension_root_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&all_extension_root_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(all_extension_root_facet["result"]["total"], 2);
        assert!(all_extension_root_facet["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let invalid_root_facet_extension_request = json!({
            "type":"request",
            "id":"root-facet-extension-invalid",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{"extension":"bin"},
                "cursor":root_facet_cursor
            }
        });
        let invalid_root_facet_extension: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_root_facet_extension_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            invalid_root_facet_extension["error"]["code"],
            "invalid_cursor"
        );
        let drive_facet_request = json!({
            "type":"request",
            "id":"drive-facet",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":false}
            }
        });
        let drive_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&drive_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(drive_facet["result"]["total"], 2);
        assert_eq!(drive_facet["result"]["facets"][0]["value"], "D:");
        assert_eq!(drive_facet["result"]["facets"][0]["matchingGroupCount"], 1);
        let minimum_copy_count_drive_facet_request = json!({
            "type":"request",
            "id":"drive-facet-minimum-copy-count",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{"minimumCopyCount":3}
            }
        });
        let minimum_copy_count_drive_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&minimum_copy_count_drive_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(minimum_copy_count_drive_facet["result"]["total"], 2);
        assert!(minimum_copy_count_drive_facet["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let extension_drive_facet_request = json!({
            "type":"request",
            "id":"drive-facet-extension",
            "method":"duplicate_file_drive_facet.page",
            "params":{"runId":1,"pageSize":25,"filter":{"extension":"bin"}}
        });
        let extension_drive_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&extension_drive_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(extension_drive_facet["result"]["total"], 2);
        assert!(extension_drive_facet["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let all_extension_drive_facet_request = json!({
            "type":"request",
            "id":"drive-facet-all-extension",
            "method":"duplicate_file_drive_facet.page",
            "params":{"runId":1,"pageSize":25,"filter":{"extension":"bin","extensionMatch":"all"}}
        });
        let all_extension_drive_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&all_extension_drive_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(all_extension_drive_facet["result"]["total"], 2);
        assert!(all_extension_drive_facet["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let drive_facet_cursor = drive_facet["result"]["nextCursor"].as_str().unwrap();
        let next_drive_facet_request = json!({
            "type":"request",
            "id":"drive-facet-next",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":false},
                "cursor":drive_facet_cursor
            }
        });
        let next_drive_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&next_drive_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(next_drive_facet["result"]["facets"][0]["value"], "E:");
        assert!(next_drive_facet["result"]["previousCursor"].is_string());
        let invalid_drive_facet_request = json!({
            "type":"request",
            "id":"drive-facet-invalid",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{
                    "search":"",
                    "minimumSize":"0",
                    "acrossDrives":false,
                    "selectedRoot":"/archive"
                },
                "cursor":drive_facet_cursor
            }
        });
        let invalid_drive_facet: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_drive_facet_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_drive_facet["error"]["code"], "invalid_cursor");
        let invalid_drive_facet_copy_count_request = json!({
            "type":"request",
            "id":"drive-facet-copy-count-invalid",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{"minimumCopyCount":3},
                "cursor":drive_facet_cursor
            }
        });
        let invalid_drive_facet_copy_count: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_drive_facet_copy_count_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            invalid_drive_facet_copy_count["error"]["code"],
            "invalid_cursor"
        );
        let invalid_drive_facet_size_request = json!({
            "type":"request",
            "id":"drive-facet-size-invalid",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{"minimumSize":"200"},
                "cursor":drive_facet_cursor
            }
        });
        let invalid_drive_facet_size: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_drive_facet_size_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_drive_facet_size["error"]["code"], "invalid_cursor");
        let invalid_drive_facet_extension_request = json!({
            "type":"request",
            "id":"drive-facet-extension-invalid",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"value","direction":"ascending"},
                "filter":{"extension":"bin"},
                "cursor":drive_facet_cursor
            }
        });
        let invalid_drive_facet_extension: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_drive_facet_extension_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            invalid_drive_facet_extension["error"]["code"],
            "invalid_cursor"
        );
        let invalid_root_facet_drive_request = json!({
            "type":"request",
            "id":"root-facet-drive-invalid",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"matchingGroupCount","direction":"descending"},
                "filter":{
                    "search":"",
                    "minimumSize":"0",
                    "acrossDrives":false,
                    "selectedDrive":"D:"
                },
                "cursor":root_facet_cursor
            }
        });
        let invalid_root_facet_drive: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_root_facet_drive_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_root_facet_drive["error"]["code"], "invalid_cursor");
        let cursor = first["result"]["nextCursor"].as_str().unwrap();
        let invalid_group_extension_cursor_request = json!({
            "type":"request",
            "id":"group-extension-cursor-invalid",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"extension":"bin"},
                "cursor":cursor
            }
        });
        let invalid_group_extension_cursor: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_group_extension_cursor_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            invalid_group_extension_cursor["error"]["code"],
            "invalid_cursor"
        );
        let exact_path_request = json!({
            "type":"request",
            "id":"exact-path",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"search":"/ROOT/B-COPY.BIN","pathMatch":"exact"}
            }
        });
        let exact_path: Value =
            serde_json::from_str(&worker.handle_line(&exact_path_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(exact_path["result"]["total"], 1);
        assert_eq!(exact_path["result"]["summary"]["matchingGroupCount"], 1);
        assert_eq!(exact_path["result"]["groups"][0]["groupSize"], "200");

        let exact_root_facets_request = json!({
            "type":"request",
            "id":"exact-path-roots",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "filter":{"search":"/ROOT/B-COPY.BIN","pathMatch":"exact"}
            }
        });
        let exact_root_facets: Value = serde_json::from_str(
            &worker
                .handle_line(&exact_root_facets_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(exact_root_facets["result"]["total"], 2);
        assert!(exact_root_facets["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let exact_root_cursor = exact_root_facets["result"]["nextCursor"].as_str().unwrap();
        let wrong_root_path_match_request = json!({
            "type":"request",
            "id":"wrong-root-path-match-cursor",
            "method":"duplicate_file_selected_root_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "filter":{"search":"/ROOT/B-COPY.BIN","pathMatch":"substring"},
                "cursor":exact_root_cursor
            }
        });
        let wrong_root_path_match: Value = serde_json::from_str(
            &worker
                .handle_line(&wrong_root_path_match_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(wrong_root_path_match["error"]["code"], "invalid_cursor");

        let exact_drive_facets_request = json!({
            "type":"request",
            "id":"exact-path-drives",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "filter":{"search":"/ROOT/B-COPY.BIN","pathMatch":"exact"}
            }
        });
        let exact_drive_facets: Value = serde_json::from_str(
            &worker
                .handle_line(&exact_drive_facets_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(exact_drive_facets["result"]["total"], 2);
        assert!(exact_drive_facets["result"]["facets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|facet| facet["matchingGroupCount"] == 1));
        let exact_drive_cursor = exact_drive_facets["result"]["nextCursor"].as_str().unwrap();
        let wrong_drive_path_match_request = json!({
            "type":"request",
            "id":"wrong-drive-path-match-cursor",
            "method":"duplicate_file_drive_facet.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "filter":{"search":"/ROOT/B-COPY.BIN","pathMatch":"substring"},
                "cursor":exact_drive_cursor
            }
        });
        let wrong_drive_path_match: Value = serde_json::from_str(
            &worker
                .handle_line(&wrong_drive_path_match_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(wrong_drive_path_match["error"]["code"], "invalid_cursor");

        let substring_cursor_request = json!({
            "type":"request",
            "id":"substring-cursor",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "filter":{"search":"/root","pathMatch":"substring"}
            }
        });
        let substring_cursor_page: Value = serde_json::from_str(
            &worker
                .handle_line(&substring_cursor_request.to_string())
                .unwrap(),
        )
        .unwrap();
        let substring_cursor = substring_cursor_page["result"]["nextCursor"]
            .as_str()
            .unwrap();
        let wrong_path_match_request = json!({
            "type":"request",
            "id":"wrong-path-match-cursor",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "filter":{"search":"/root","pathMatch":"exact"},
                "cursor":substring_cursor
            }
        });
        let wrong_path_match: Value = serde_json::from_str(
            &worker
                .handle_line(&wrong_path_match_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(wrong_path_match["error"]["code"], "invalid_cursor");

        let invalid_path_match_request = json!({
            "type":"request",
            "id":"invalid-path-match",
            "method":"duplicate_file_group.page",
            "params":{"runId":1,"filter":{"pathMatch":"prefix"}}
        });
        let invalid_path_match: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_path_match_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_path_match["error"]["code"], "invalid_request");
        assert_eq!(
            invalid_path_match["error"]["details"]["field"],
            "filter.pathMatch"
        );
        let second_request = json!({
            "type":"request",
            "id":"second",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"search":"","minimumSize":"0"},
                "cursor":cursor,
            }
        });
        let second: Value =
            serde_json::from_str(&worker.handle_line(&second_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(second["result"]["groups"][0]["groupSize"], "100");
        assert!(second["result"]["previousCursor"].is_string());

        let invalid_request = json!({
            "type":"request",
            "id":"invalid",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"search":"","minimumSize":"0","acrossDrives":true},
                "cursor":cursor,
            }
        });
        let invalid: Value =
            serde_json::from_str(&worker.handle_line(&invalid_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_cursor");
        let invalid_copy_count_cursor_request = json!({
            "type":"request",
            "id":"invalid-copy-count-cursor",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"minimumCopyCount":3},
                "cursor":cursor,
            }
        });
        let invalid_copy_count_cursor: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_copy_count_cursor_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_copy_count_cursor["error"]["code"], "invalid_cursor");
        let invalid_size_cursor_request = json!({
            "type":"request",
            "id":"invalid-size-cursor",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{"minimumSize":"200"},
                "cursor":cursor
            }
        });
        let invalid_size_cursor: Value = serde_json::from_str(
            &worker
                .handle_line(&invalid_size_cursor_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(invalid_size_cursor["error"]["code"], "invalid_cursor");

        let selected_root_request = json!({
            "type":"request",
            "id":"selected-root",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{
                    "search":"",
                    "minimumSize":"0",
                    "acrossDrives":false,
                    "selectedRoot":"/ARCHIVE"
                }
            }
        });
        let selected_root: Value = serde_json::from_str(
            &worker
                .handle_line(&selected_root_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(selected_root["result"]["total"], 1);
        assert_eq!(selected_root["result"]["summary"]["matchingGroupCount"], 1);
        assert_eq!(selected_root["result"]["groups"][0]["groupSize"], "200");

        let selected_drive_request = json!({
            "type":"request",
            "id":"selected-drive",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":25,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{
                    "search":"",
                    "minimumSize":"0",
                    "acrossDrives":false,
                    "selectedDrive":"e:"
                }
            }
        });
        let selected_drive: Value = serde_json::from_str(
            &worker
                .handle_line(&selected_drive_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(selected_drive["result"]["total"], 1);
        assert_eq!(selected_drive["result"]["summary"]["matchingGroupCount"], 1);
        assert_eq!(selected_drive["result"]["groups"][0]["groupSize"], "200");

        let selected_root_cursor_request = json!({
            "type":"request",
            "id":"selected-root-cursor",
            "method":"duplicate_file_group.page",
            "params":{
                "runId":1,
                "pageSize":1,
                "sort":{"field":"recoverableBytes","direction":"descending"},
                "filter":{
                    "search":"",
                    "minimumSize":"0",
                    "selectedRoot":"/archive"
                },
                "cursor":cursor
            }
        });
        let selected_root_cursor: Value = serde_json::from_str(
            &worker
                .handle_line(&selected_root_cursor_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(selected_root_cursor["error"]["code"], "invalid_cursor");

        let group_id = first["result"]["groups"][0]["id"].as_i64().unwrap();
        let members_request = json!({
            "type":"request",
            "id":"members",
            "method":"duplicate_file_group.members",
            "params":{"runId":1,"groupId":group_id,"pageSize":200}
        });
        let members: Value =
            serde_json::from_str(&worker.handle_line(&members_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(members["result"]["members"].as_array().unwrap().len(), 3);
        assert!(members["result"]["members"][0]["modifiedTimeUnixNanos"].is_string());
        assert!(members["result"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|member| member["rootPath"] == "/archive"
                && member["relativePath"] == "copies/b-copy.bin"
                && member["driveLetter"] == "E:"));

        let initial_plan: Value = serde_json::from_str(
            &worker
                .handle_line(
                    r#"{"type":"request","id":"review-initial","method":"review_plan.get","params":{"runId":1}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert!(initial_plan["result"]["plan"]["id"].is_null());
        assert_eq!(initial_plan["result"]["plan"]["revision"], 0);
        assert_eq!(initial_plan["result"]["summary"]["undecidedCount"], 5);
        let review_groups: Value = serde_json::from_str(
            &worker
                .handle_line(
                    r#"{"type":"request","id":"review-groups","method":"review_group.page","params":{"runId":1,"pageSize":1}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            review_groups["result"]["groups"].as_array().unwrap().len(),
            1
        );
        let stale_review_cursor = review_groups["result"]["nextCursor"]
            .as_str()
            .unwrap()
            .to_owned();
        let member_ids = members["result"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| member["id"].as_i64().unwrap())
            .collect::<Vec<_>>();
        let set_remove = json!({
            "type":"request", "id":"review-remove", "method":"review_decision.set",
            "params":{"operationId":"remove-one","runId":1,"groupId":group_id,
                      "fileId":member_ids[0],"decision":"remove","expectedRevision":0}
        });
        let removed: Value =
            serde_json::from_str(&worker.handle_line(&set_remove.to_string()).unwrap()).unwrap();
        assert_eq!(removed["result"]["appliedRevision"], 1);
        assert_eq!(removed["result"]["replayed"], false);
        let replayed: Value =
            serde_json::from_str(&worker.handle_line(&set_remove.to_string()).unwrap()).unwrap();
        assert_eq!(replayed["result"]["replayed"], true);
        let conflict_request = json!({
            "type":"request", "id":"review-conflict", "method":"review_decision.set",
            "params":{"operationId":"remove-one","runId":1,"groupId":group_id,
                      "fileId":member_ids[1],"decision":"keep","expectedRevision":0}
        });
        let conflict: Value =
            serde_json::from_str(&worker.handle_line(&conflict_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(conflict["error"]["code"], "idempotency_conflict");
        let stale_request = json!({
            "type":"request", "id":"review-stale", "method":"review_decision.set",
            "params":{"operationId":"stale","runId":1,"groupId":group_id,
                      "fileId":member_ids[1],"decision":"keep","expectedRevision":0}
        });
        let stale: Value =
            serde_json::from_str(&worker.handle_line(&stale_request.to_string()).unwrap()).unwrap();
        assert_eq!(stale["error"]["code"], "review_generation_conflict");
        let remove_second_request = json!({
            "type":"request", "id":"review-remove-second", "method":"review_decision.set",
            "params":{"operationId":"remove-two","runId":1,"groupId":group_id,
                      "fileId":member_ids[1],"decision":"remove","expectedRevision":1}
        });
        let removed_second: Value = serde_json::from_str(
            &worker
                .handle_line(&remove_second_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(removed_second["result"]["appliedRevision"], 2);
        let unsafe_request = json!({
            "type":"request", "id":"review-unsafe", "method":"review_decision.set",
            "params":{"operationId":"remove-three","runId":1,"groupId":group_id,
                      "fileId":member_ids[2],"decision":"remove","expectedRevision":2}
        });
        let unsafe_response: Value =
            serde_json::from_str(&worker.handle_line(&unsafe_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(unsafe_response["error"]["code"], "unsafe_review_decision");
        let stale_cursor_request = json!({
            "type":"request", "id":"review-old-cursor", "method":"review_group.page",
            "params":{"runId":1,"pageSize":1,"cursor":stale_review_cursor}
        });
        let stale_cursor_response: Value = serde_json::from_str(
            &worker
                .handle_line(&stale_cursor_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(stale_cursor_response["error"]["code"], "invalid_cursor");
        let reviewed_members: Value =
            serde_json::from_str(&worker.handle_line(&members_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(reviewed_members["result"]["reviewRevision"], 2);
        assert_eq!(
            reviewed_members["result"]["reviewSummary"]["removeCount"],
            2
        );
        assert_eq!(
            reviewed_members["result"]["members"][0]["decision"],
            "remove"
        );
        assert_eq!(
            reviewed_members["result"]["members"][0]["decisionProvenance"],
            "manual"
        );

        drop(worker);
        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let mut restarted = WorkerSession::new(state);
        restarted.handle_line(HELLO).unwrap();
        let restored: Value = serde_json::from_str(
            &restarted
                .handle_line(
                    r#"{"type":"request","id":"review-restored","method":"review_plan.get","params":{"runId":1}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(restored["result"]["plan"]["revision"], 2);
        assert_eq!(restored["result"]["summary"]["removeCount"], 2);
    }

    #[test]
    fn duplicate_folder_pages_are_stable_filtered_and_run_owned() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("folder-worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        let session_id = db
            .create_session("Folders", &["/root".to_owned()], &[])
            .unwrap();
        let run_id = db
            .create_scan_run(
                session_id,
                &RunParameters {
                    roots: vec!["/root".to_owned()],
                    ignore_patterns: vec![],
                    directory_similarity_threshold_millis: 500,
                    cloud_policy: Default::default(),
                    manual_location_exclusions: vec![],
                    registered_cloud_locations: vec![],
                    cloud_detection_status: Default::default(),
                },
                "test",
            )
            .unwrap();
        db.start_scan_run(run_id).unwrap();
        let directories = [
            "/root/a", "/root/b", "/root/c", "/root/d", "/root/e", "/root/f",
        ]
        .into_iter()
        .map(|path| {
            db.insert_directory_node(run_id, path, path, None, 100, 1, 1)
                .unwrap()
        })
        .collect::<Vec<_>>();
        db.replace_exact_folder_groups(
            run_id,
            &[
                super_duper_core::storage::models::ExactFolderGroupInsert {
                    structural_fingerprint: "s1".to_owned(),
                    verified_fingerprint: "v1".to_owned(),
                    total_size: 100,
                    file_count: 1,
                    directory_ids: directories[0..2].to_vec(),
                    is_suppressed: false,
                },
                super_duper_core::storage::models::ExactFolderGroupInsert {
                    structural_fingerprint: "s2".to_owned(),
                    verified_fingerprint: "v2".to_owned(),
                    total_size: 100,
                    file_count: 1,
                    directory_ids: directories[2..4].to_vec(),
                    is_suppressed: false,
                },
                super_duper_core::storage::models::ExactFolderGroupInsert {
                    structural_fingerprint: "s3".to_owned(),
                    verified_fingerprint: "v3".to_owned(),
                    total_size: 100,
                    file_count: 1,
                    directory_ids: directories[4..6].to_vec(),
                    is_suppressed: false,
                },
            ],
            &AtomicBool::new(false),
        )
        .unwrap();
        db.complete_scan_run(run_id, 6, 600, 6, 0, 3, 0, 0).unwrap();
        drop(db);

        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let mut worker = WorkerSession::new(state);
        worker.handle_line(HELLO).unwrap();
        let first: Value = serde_json::from_str(&worker.handle_line(
            r#"{"type":"request","id":"fg1","method":"duplicate_folder_group.page","params":{"runId":1,"pageSize":2,"sort":{"field":"totalBytes","direction":"descending"},"filter":{"search":"","minimumSize":"0"}}}"#,
        ).unwrap()).unwrap();
        assert_eq!(first["result"]["groups"].as_array().unwrap().len(), 2);
        assert_eq!(first["result"]["groups"][0]["id"], 1);
        assert_eq!(first["result"]["groups"][1]["id"], 2);
        assert!(first["result"]["previousCursor"].is_null());
        let cursor = first["result"]["nextCursor"].as_str().unwrap();
        let second_request = json!({
            "type":"request", "id":"fg2", "method":"duplicate_folder_group.page",
            "params":{"runId":1,"pageSize":2,"sort":{"field":"totalBytes","direction":"descending"},
                      "filter":{"search":"","minimumSize":"0"},"cursor":cursor}
        });
        let second: Value =
            serde_json::from_str(&worker.handle_line(&second_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(second["result"]["groups"][0]["id"], 3);

        let invalid_request = json!({
            "type":"request", "id":"bad", "method":"duplicate_folder_group.page",
            "params":{"runId":1,"pageSize":2,"sort":{"field":"totalBytes","direction":"descending"},
                      "filter":{"search":"a","minimumSize":"0"},"cursor":cursor}
        });
        let invalid: Value =
            serde_json::from_str(&worker.handle_line(&invalid_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(invalid["error"]["code"], "invalid_cursor");

        let members: Value = serde_json::from_str(&worker.handle_line(
            r#"{"type":"request","id":"fm","method":"duplicate_folder_group.members","params":{"runId":1,"groupId":1,"pageSize":200}}"#,
        ).unwrap()).unwrap();
        assert_eq!(members["result"]["members"].as_array().unwrap().len(), 2);
        assert!(members["result"]["members"][0]["path"].is_string());
        assert_eq!(members["result"]["reviewRevision"], 0);
        assert_eq!(members["result"]["reviewSummary"]["undecidedCount"], 2);
        let folder_review: Value = serde_json::from_str(
            &worker
                .handle_line(
                    r#"{"type":"request","id":"rfg","method":"review_folder_group.page","params":{"runId":1,"pageSize":2}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            folder_review["result"]["groups"].as_array().unwrap().len(),
            2
        );
        let stale_folder_review_cursor = folder_review["result"]["nextCursor"]
            .as_str()
            .unwrap()
            .to_owned();
        let first_member_page: Value = serde_json::from_str(
            &worker
                .handle_line(
                    r#"{"type":"request","id":"fm-one","method":"duplicate_folder_group.members","params":{"runId":1,"groupId":1,"pageSize":1}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        let stale_member_cursor = first_member_page["result"]["nextCursor"]
            .as_str()
            .unwrap()
            .to_owned();
        let folder_member_id = first_member_page["result"]["members"][0]["id"]
            .as_i64()
            .unwrap();
        let set_folder = json!({
            "type":"request", "id":"folder-remove", "method":"review_folder_decision.set",
            "params":{"operationId":"folder-remove-one","runId":1,"folderGroupId":1,
                      "folderMemberId":folder_member_id,"decision":"remove","expectedRevision":0}
        });
        let removed: Value =
            serde_json::from_str(&worker.handle_line(&set_folder.to_string()).unwrap()).unwrap();
        assert_eq!(removed["result"]["appliedRevision"], 1);
        let replayed: Value =
            serde_json::from_str(&worker.handle_line(&set_folder.to_string()).unwrap()).unwrap();
        assert_eq!(replayed["result"]["replayed"], true);
        let old_review_cursor = json!({
            "type":"request", "id":"old-rfg", "method":"review_folder_group.page",
            "params":{"runId":1,"pageSize":2,"cursor":stale_folder_review_cursor}
        });
        let old_review: Value =
            serde_json::from_str(&worker.handle_line(&old_review_cursor.to_string()).unwrap())
                .unwrap();
        assert_eq!(old_review["error"]["code"], "invalid_cursor");
        let old_member_cursor = json!({
            "type":"request", "id":"old-fm", "method":"duplicate_folder_group.members",
            "params":{"runId":1,"groupId":1,"pageSize":1,"cursor":stale_member_cursor}
        });
        let old_member: Value =
            serde_json::from_str(&worker.handle_line(&old_member_cursor.to_string()).unwrap())
                .unwrap();
        assert_eq!(old_member["error"]["code"], "invalid_cursor");
        let refreshed_members: Value = serde_json::from_str(
            &worker
                .handle_line(
                    r#"{"type":"request","id":"fm-refreshed","method":"duplicate_folder_group.members","params":{"runId":1,"groupId":1,"pageSize":200}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(refreshed_members["result"]["reviewRevision"], 1);
        assert_eq!(
            refreshed_members["result"]["reviewSummary"]["removeCount"],
            1
        );
        assert_eq!(
            refreshed_members["result"]["members"][0]["decision"],
            "remove"
        );
    }

    #[test]
    fn second_start_is_busy_and_cancellation_reaches_durable_cancelled_state() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("scan");
        fs::create_dir(&root).unwrap();
        for index in 0..200 {
            fs::write(root.join(format!("{index}.bin")), vec![index as u8; 4096]).unwrap();
        }
        let db_path = temp.path().join("worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        db.create_session_with_cloud_settings(
            "Scan",
            &[root.to_string_lossy().into_owned()],
            &[],
            CloudPolicy::ExcludeRegisteredRoots,
            &[],
            &[],
            CloudDetectionStatus::Complete,
        )
        .unwrap();
        drop(db);
        let frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                r#"{"type":"request","id":"start","method":"run.start","params":{"sessionId":1}}"#
                    .to_owned(),
                r#"{"type":"request","id":"busy","method":"run.start","params":{"sessionId":1}}"#
                    .to_owned(),
                r#"{"type":"request","id":"mutate","method":"session.delete","params":{"sessionId":1}}"#
                    .to_owned(),
                r#"{"type":"request","id":"cancel","method":"run.cancel","params":{"runId":1}}"#
                    .to_owned(),
                r#"{"type":"request","id":"get","method":"run.get","params":{"runId":1}}"#
                    .to_owned(),
            ],
        );
        assert_eq!(response(&frames, "start")["result"]["run"]["id"], 1);
        assert_eq!(response(&frames, "busy")["error"]["code"], "scan_busy");
        assert_eq!(
            response(&frames, "mutate")["error"]["code"],
            "invalid_state"
        );
        assert_eq!(
            response(&frames, "cancel")["result"]["run"]["status"],
            "cancelling"
        );
        assert!(
            frames.iter().any(|frame| {
                frame["type"] == "event"
                    && frame["event"] == "run.cancelled"
                    && frame["data"]["run"]["status"] == "cancelled"
            }),
            "frames: {frames:#?}"
        );
        let reopened = Database::open_connection(db_path.to_str().unwrap()).unwrap();
        assert_eq!(reopened.get_scan_run(1).unwrap().status, "cancelled");
    }

    #[test]
    fn delayed_latest_progress_emits_without_another_callback_and_stops_at_terminal() {
        use super_duper_core::telemetry::{
            ActiveDeviceProgress, ActiveDeviceUnavailableReason, ProgressLogicalCounters,
            ProgressObservation, ScanCounters, METRICS_CONTRACT_VERSION, PROGRESS_CONTRACT_VERSION,
        };

        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let (sender, receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let cancel_token = Arc::new(AtomicBool::new(false));
        let reporter = WorkerProgressReporter::new(state, 41, cancel_token.clone());
        let observation = |monotonic_nanos, discovered_files, zero_byte_files, discovered_bytes| {
            let mut counters = ScanCounters::default();
            counters.discovered_files = discovered_files;
            counters.zero_byte_files = zero_byte_files;
            counters.discovered_bytes = discovered_bytes;
            ProgressObservation {
                progress_contract_version: PROGRESS_CONTRACT_VERSION,
                metrics_contract_version: METRICS_CONTRACT_VERSION,
                monotonic_nanos,
                phase: TelemetryPhase::Discovering,
                phase_started_monotonic_nanos: 0,
                candidate_totals_known: false,
                final_results_complete: false,
                counters,
                logical: ProgressLogicalCounters::default(),
                active_devices: ActiveDeviceProgress::Unavailable {
                    reason: ActiveDeviceUnavailableReason::MappingUnavailable,
                },
            }
        };

        reporter.on_progress_observation(&observation(0, 0, 0, 0));
        let first: Value = serde_json::from_str(
            &receiver
                .recv_timeout(Duration::from_millis(500))
                .expect("first progress frame"),
        )
        .unwrap();
        assert_eq!(first["data"]["sequence"], 1);

        reporter.on_progress_observation(&observation(1, 2, 1, u64::MAX));
        let delayed: Value = serde_json::from_str(
            &receiver
                .recv_timeout(Duration::from_millis(500))
                .expect("timed coalescer must wake without another callback"),
        )
        .unwrap();
        assert_eq!(delayed["data"]["sequence"], 2);
        assert_eq!(delayed["data"]["filesDiscovered"], 1);
        assert_eq!(
            delayed["data"]["progress"]["counters"]["discoveredFiles"],
            2
        );
        assert_eq!(
            delayed["data"]["progress"]["counters"]["discoveredBytes"],
            u64::MAX.to_string()
        );

        reporter.on_progress_observation(&observation(2, 3, 1, u64::MAX));
        cancel_token.store(true, Ordering::Release);
        let cancelling: Value = serde_json::from_str(
            &receiver
                .recv_timeout(Duration::from_millis(500))
                .expect("pending progress must observe cancellation before emission"),
        )
        .unwrap();
        assert_eq!(cancelling["data"]["sequence"], 3);
        assert_eq!(cancelling["data"]["status"], "cancelling");

        reporter.finish_progress();
        reporter.on_progress_observation(&observation(3, 4, 1, u64::MAX));
        assert!(receiver.recv_timeout(Duration::from_millis(150)).is_err());
    }

    #[test]
    fn warning_progress_persists_exact_bounded_accounting_before_publication() {
        use super_duper_core::storage::models::{
            RunWarningPageQuery, RunWarningSortField, SortDirection,
        };
        use super_duper_core::telemetry::{
            ActiveDeviceProgress, ActiveDeviceUnavailableReason, ProgressLogicalCounters,
            ProgressObservation, ScanCounters, METRICS_CONTRACT_VERSION, PROGRESS_CONTRACT_VERSION,
        };

        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let (sender, receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
        let session_id = db
            .create_session("Live warnings", &["/root".into()], &[])
            .unwrap();
        let parameters = RunParameters {
            roots: vec!["/root".into()],
            ignore_patterns: vec![],
            directory_similarity_threshold_millis: 500,
            cloud_policy: Default::default(),
            manual_location_exclusions: vec![],
            registered_cloud_locations: vec![],
            cloud_detection_status: Default::default(),
        };
        let run_id = db.create_scan_run(session_id, &parameters, "test").unwrap();
        db.start_scan_run(run_id).unwrap();
        drop(db);

        let reporter = WorkerProgressReporter::new(state, run_id, Arc::new(AtomicBool::new(false)));
        let mut counters = ScanCounters::default();
        counters.discovered_files = 1;
        counters.discovered_bytes = 4_096;
        counters.warnings = 3;
        reporter.on_progress_observation(&ProgressObservation {
            progress_contract_version: PROGRESS_CONTRACT_VERSION,
            metrics_contract_version: METRICS_CONTRACT_VERSION,
            monotonic_nanos: 1,
            phase: TelemetryPhase::Discovering,
            phase_started_monotonic_nanos: 0,
            candidate_totals_known: false,
            final_results_complete: false,
            counters,
            logical: ProgressLogicalCounters::default(),
            active_devices: ActiveDeviceProgress::Unavailable {
                reason: ActiveDeviceUnavailableReason::MappingUnavailable,
            },
        });

        let frame: Value = serde_json::from_str(
            &receiver
                .recv_timeout(Duration::from_millis(500))
                .expect("warning progress frame"),
        )
        .unwrap();
        assert_eq!(frame["data"]["warningCount"], 3);
        let persisted = Database::open_connection(db_path.to_str().unwrap()).unwrap();
        let run = persisted.get_scan_run(run_id).unwrap();
        assert_eq!(run.warning_count, 3);
        let (warnings, total, accounted) = persisted
            .page_run_warning_aggregates(&RunWarningPageQuery {
                run_id,
                limit: 25,
                sort_field: RunWarningSortField::OccurrenceCount,
                sort_direction: SortDirection::Descending,
                cursor: None,
            })
            .unwrap();
        assert_eq!((warnings.len() as i64, total, accounted), (1, 1, 3));
        assert_eq!(warnings[0].code, "active_unclassified_recoverable_warning");
        reporter.finish_progress();
    }

    #[test]
    fn warning_progress_is_silent_when_durable_accounting_fails() {
        use super_duper_core::telemetry::{
            ActiveDeviceProgress, ActiveDeviceUnavailableReason, ProgressLogicalCounters,
            ProgressObservation, ScanCounters, METRICS_CONTRACT_VERSION, PROGRESS_CONTRACT_VERSION,
        };

        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let (sender, receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let reporter = WorkerProgressReporter::new(state, 41, Arc::new(AtomicBool::new(false)));
        let mut counters = ScanCounters::default();
        counters.warnings = 1;
        reporter.on_progress_observation(&ProgressObservation {
            progress_contract_version: PROGRESS_CONTRACT_VERSION,
            metrics_contract_version: METRICS_CONTRACT_VERSION,
            monotonic_nanos: 1,
            phase: TelemetryPhase::Discovering,
            phase_started_monotonic_nanos: 0,
            candidate_totals_known: false,
            final_results_complete: false,
            counters,
            logical: ProgressLogicalCounters::default(),
            active_devices: ActiveDeviceProgress::Unavailable {
                reason: ActiveDeviceUnavailableReason::MappingUnavailable,
            },
        });

        assert!(receiver.recv_timeout(Duration::from_millis(150)).is_err());
        reporter.finish_progress();
    }

    #[test]
    fn completed_run_emits_ordered_coalesced_progress_before_matching_terminal_state() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("scan");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("one.bin"), b"one non-empty file").unwrap();
        let db_path = temp.path().join("worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        db.create_session_with_cloud_settings(
            "Scan",
            &[root.to_string_lossy().into_owned()],
            &[],
            CloudPolicy::ExcludeRegisteredRoots,
            &[],
            &[],
            CloudDetectionStatus::Complete,
        )
        .unwrap();
        drop(db);

        let (sender, receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender.clone()).unwrap();
        let mut session = WorkerSession::new(state.clone());
        assert_eq!(
            serde_json::from_str::<Value>(&session.handle_line(HELLO).unwrap()).unwrap()["ok"],
            true
        );
        let start = session
            .handle_line(
                r#"{"type":"request","id":"start","method":"run.start","params":{"sessionId":1}}"#,
            )
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&start).unwrap()["ok"], true);

        let mut active = state.active.lock().unwrap();
        while active.is_some() {
            active = state.idle.wait(active).unwrap();
        }
        drop(active);
        drop(session);
        drop(state);
        drop(sender);
        let events: Vec<Value> = receiver
            .into_iter()
            .map(|line| serde_json::from_str(&line).unwrap())
            .collect();
        let progress: Vec<&Value> = events
            .iter()
            .filter(|frame| frame["event"] == "run.progress")
            .collect();
        let phases: Vec<&str> = progress
            .iter()
            .filter_map(|frame| frame["data"]["phase"].as_str())
            .collect();
        assert_eq!(phases.first().copied(), Some("discovering"));
        let phase_order = |phase: &str| match phase {
            "discovering" => 0,
            "hashing" => 1,
            "persisting" => 2,
            "analyzing_folders" => 3,
            "finalizing" => 4,
            _ => panic!("unexpected progress phase {phase}"),
        };
        assert!(phases
            .windows(2)
            .all(|pair| phase_order(pair[0]) <= phase_order(pair[1])));
        assert!(progress.windows(2).all(|pair| {
            pair[0]["data"]["sequence"].as_u64().unwrap()
                < pair[1]["data"]["sequence"].as_u64().unwrap()
        }));
        assert!(progress.iter().all(|frame| {
            frame["data"]["progress"]["progressContractVersion"] == PROGRESS_CONTRACT_VERSION
                && frame["data"]["progress"]["metricsContractVersion"] == METRICS_CONTRACT_VERSION
        }));
        let terminal_index = events
            .iter()
            .position(|frame| {
                frame["event"] == "run.completed" && frame["data"]["run"]["status"] == "completed"
            })
            .expect("completed terminal event");
        assert!(events[terminal_index + 1..]
            .iter()
            .all(|frame| frame["event"] != "run.progress"));
        let reopened = Database::open_connection(db_path.to_str().unwrap()).unwrap();
        assert_eq!(reopened.get_scan_run(1).unwrap().status, "completed");
    }

    #[test]
    fn default_cloud_policy_fails_closed_when_detection_is_unavailable() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        db.create_session(
            "Unverified",
            &[temp.path().to_string_lossy().into_owned()],
            &[],
        )
        .unwrap();
        drop(db);
        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let mut session = WorkerSession::new(state);
        session.handle_line(HELLO).unwrap();

        let response: Value = serde_json::from_str(
            &session
                .handle_line(
                    r#"{"type":"request","id":"start","method":"run.start","params":{"sessionId":1}}"#,
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(response["error"]["code"], "invalid_session");
        assert_eq!(
            response["error"]["details"]["field"],
            "cloudDetectionStatus"
        );
    }

    #[test]
    fn completed_run_snapshots_and_pages_excluded_cloud_subtrees() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("scan");
        let local = root.join("local");
        let cloud = root.join("cloud");
        fs::create_dir_all(&local).unwrap();
        fs::create_dir_all(&cloud).unwrap();
        fs::write(local.join("kept.bin"), b"kept").unwrap();
        fs::write(cloud.join("excluded.bin"), b"excluded").unwrap();
        let db_path = temp.path().join("worker.db");
        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let mut session = WorkerSession::new(state.clone());
        session.handle_line(HELLO).unwrap();
        let create = json!({
            "type":"request", "id":"create", "method":"session.create",
            "params":{
                "name":"Cloud safe",
                "roots":[root.to_string_lossy()],
                "ignorePatterns":[],
                "cloudPolicy":"exclude_registered_roots",
                "manualLocationExclusions":[],
                "registeredCloudLocations":[{
                    "path":cloud.to_string_lossy(),
                    "providerId":"TestProvider!account",
                    "displayName":"TestProvider"
                }],
                "cloudDetectionStatus":"complete"
            }
        });
        let created: Value =
            serde_json::from_str(&session.handle_line(&create.to_string()).unwrap()).unwrap();
        assert_eq!(created["ok"], true);
        let started: Value = serde_json::from_str(
            &session.handle_line(
                r#"{"type":"request","id":"start","method":"run.start","params":{"sessionId":1}}"#,
            ).unwrap(),
        ).unwrap();
        assert_eq!(started["ok"], true);

        let mut active = state.active.lock().unwrap();
        while active.is_some() {
            active = state.idle.wait(active).unwrap();
        }
        drop(active);

        let run: Value = serde_json::from_str(
            &session
                .handle_line(
                    r#"{"type":"request","id":"get","method":"run.get","params":{"runId":1}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(run["result"]["run"]["filesDiscovered"], 1);
        assert_eq!(run["result"]["run"]["excludedSubtreeCount"], 1);
        assert_eq!(
            run["result"]["run"]["parameters"]["cloudPolicy"],
            "exclude_registered_roots"
        );

        let page: Value = serde_json::from_str(
            &session.handle_line(
                r#"{"type":"request","id":"excluded","method":"run_exclusion.page","params":{"runId":1,"offset":0,"limit":1}}"#,
            ).unwrap(),
        ).unwrap();
        assert_eq!(page["result"]["total"], 1);
        assert_eq!(
            page["result"]["exclusions"][0]["reasonCode"],
            "registered_cloud_root_excluded"
        );
        assert_eq!(
            page["result"]["exclusions"][0]["providerName"],
            "TestProvider"
        );
    }

    #[test]
    fn warning_protocol_pages_bounded_aggregates_rejects_stale_cursors_and_restarts() {
        use super_duper_core::storage::models::RunWarningAggregateInsert;

        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        let session_id = db
            .create_session("Warnings", &["/root".into()], &[])
            .unwrap();
        let parameters = RunParameters {
            roots: vec!["/root".into()],
            ignore_patterns: vec![],
            directory_similarity_threshold_millis: 500,
            cloud_policy: Default::default(),
            manual_location_exclusions: vec![],
            registered_cloud_locations: vec![],
            cloud_detection_status: Default::default(),
        };
        let run_id = db.create_scan_run(session_id, &parameters, "test").unwrap();
        db.start_scan_run(run_id).unwrap();
        db.replace_run_warning_aggregates(
            run_id,
            &[
                RunWarningAggregateInsert {
                    phase: "hashing".into(),
                    category: "scan".into(),
                    code: "hash_recoverable_warning".into(),
                    message: "First aggregate".into(),
                    occurrence_count: 5,
                    examples: vec!["example one".into()],
                },
                RunWarningAggregateInsert {
                    phase: "persisting".into(),
                    category: "scan".into(),
                    code: "two".into(),
                    message: "Second aggregate".into(),
                    occurrence_count: 3,
                    examples: vec!["example two".into(), "example three".into()],
                },
            ],
        )
        .unwrap();
        db.complete_scan_run(run_id, 0, 0, 0, 0, 0, 0, 8).unwrap();
        let second_run = db.create_scan_run(session_id, &parameters, "test").unwrap();
        db.start_scan_run(second_run).unwrap();
        db.complete_scan_run(second_run, 0, 0, 0, 0, 0, 0, 0)
            .unwrap();
        drop(db);

        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(WorkerOptions::new(&db_path), sender).unwrap();
        let mut session = WorkerSession::new(state);
        session.handle_line(HELLO).unwrap();
        let first: Value = serde_json::from_str(&session.handle_line(
            r#"{"type":"request","id":"warnings","method":"warning.page","params":{"runId":1,"pageSize":1}}"#,
        ).unwrap()).unwrap();
        assert_eq!(first["result"]["warningCount"], 8);
        assert_eq!(first["result"]["accountedWarningCount"], 8);
        assert_eq!(first["result"]["snapshotState"], "terminal");
        assert_eq!(first["result"]["runStatus"], "completed");
        assert!(first["result"]["snapshotRevision"].as_i64().unwrap() > 0);
        assert_eq!(first["result"]["diagnosticLog"]["state"], "unavailable");
        assert_eq!(first["result"]["total"], 2);
        assert_eq!(first["result"]["warnings"].as_array().unwrap().len(), 1);
        assert_eq!(first["result"]["warnings"][0]["runId"], run_id);
        assert_eq!(first["result"]["warnings"][0]["category"], "scan");
        assert_eq!(
            first["result"]["warnings"][0]["code"],
            "hash_recoverable_warning"
        );
        assert_eq!(first["result"]["executorEnabled"], false);
        let cursor = first["result"]["nextCursor"].as_str().unwrap();
        let next_request = json!({
            "type":"request", "id":"next", "method":"warning.page",
            "params":{"runId":1,"pageSize":1,"cursor":cursor}
        });
        let next: Value =
            serde_json::from_str(&session.handle_line(&next_request.to_string()).unwrap()).unwrap();
        assert_eq!(next["result"]["warnings"].as_array().unwrap().len(), 1);
        assert_eq!(next["result"]["warnings"][0]["code"], "two");
        let changed_sort_request = json!({
            "type":"request", "id":"changed-sort", "method":"warning.page",
            "params":{"runId":1,"pageSize":1,"sort":{"field":"phase","direction":"ascending"},"cursor":cursor}
        });
        let changed_sort: Value = serde_json::from_str(
            &session
                .handle_line(&changed_sort_request.to_string())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(changed_sort["error"]["code"], "invalid_cursor");
        let sorted_request = json!({
            "type":"request", "id":"sorted", "method":"warning.page",
            "params":{"runId":1,"pageSize":1,"sort":{"field":"phase","direction":"descending"}}
        });
        let sorted: Value =
            serde_json::from_str(&session.handle_line(&sorted_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(sorted["result"]["warnings"][0]["code"], "two");
        let stale_request = json!({
            "type":"request", "id":"stale", "method":"warning.page",
            "params":{"runId":2,"pageSize":1,"cursor":cursor}
        });
        let stale: Value =
            serde_json::from_str(&session.handle_line(&stale_request.to_string()).unwrap())
                .unwrap();
        assert_eq!(stale["error"]["code"], "invalid_cursor");
    }

    #[test]
    fn active_warning_pages_reject_mutated_snapshots_and_reconstruct_terminal_state() {
        use super_duper_core::storage::models::RunWarningAggregateInsert;

        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let diagnostics = temp.path().join("logs").join("worker.log");
        let options = WorkerOptions::new(&db_path).with_diagnostic_log_path(&diagnostics);
        let (sender, _receiver) = mpsc::channel();
        let state = SharedState::new(options.clone(), sender).unwrap();
        let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
        let session_id = db
            .create_session("Active warnings", &["/root".into()], &[])
            .unwrap();
        let parameters = RunParameters {
            roots: vec!["/root".into()],
            ignore_patterns: vec![],
            directory_similarity_threshold_millis: 500,
            cloud_policy: Default::default(),
            manual_location_exclusions: vec![],
            registered_cloud_locations: vec![],
            cloud_detection_status: Default::default(),
        };
        let run_id = db.create_scan_run(session_id, &parameters, "test").unwrap();
        db.start_scan_run(run_id).unwrap();
        db.replace_run_warning_aggregates(
            run_id,
            &[
                RunWarningAggregateInsert {
                    phase: "discovering".into(),
                    category: "scan".into(),
                    code: "one".into(),
                    message: "First active warning".into(),
                    occurrence_count: 5,
                    examples: vec!["first".into()],
                },
                RunWarningAggregateInsert {
                    phase: "hashing".into(),
                    category: "scan".into(),
                    code: "two".into(),
                    message: "Second active warning".into(),
                    occurrence_count: 3,
                    examples: vec!["second".into()],
                },
            ],
        )
        .unwrap();
        drop(db);

        let mut session = WorkerSession::new(state);
        session.handle_line(HELLO).unwrap();
        let first: Value = serde_json::from_str(
            &session
                .handle_line(
                    &json!({
                        "type": "request", "id": "active", "method": "warning.page",
                        "params": {"runId": run_id, "pageSize": 1}
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(first["result"]["snapshotState"], "active");
        assert_eq!(first["result"]["runStatus"], "running");
        assert_eq!(first["result"]["diagnosticLog"]["state"], "available");
        assert_eq!(
            first["result"]["diagnosticLog"]["path"],
            diagnostics.to_string_lossy().as_ref()
        );
        let first_revision = first["result"]["snapshotRevision"].as_i64().unwrap();
        let active_cursor = first["result"]["nextCursor"].as_str().unwrap().to_owned();

        let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
        db.update_run_progress_with_warning_accounting(run_id, "hashing", 1, 1, 0, 9)
            .unwrap();
        drop(db);
        let stale: Value = serde_json::from_str(
            &session
                .handle_line(
                    &json!({
                        "type": "request", "id": "stale-active", "method": "warning.page",
                        "params": {"runId": run_id, "pageSize": 1, "cursor": active_cursor}
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(stale["error"]["code"], "invalid_cursor");
        drop(session);

        let (sender, _receiver) = mpsc::channel();
        let restarted_state = SharedState::new(options, sender).unwrap();
        let mut restarted = WorkerSession::new(restarted_state);
        restarted.handle_line(HELLO).unwrap();
        let restored: Value = serde_json::from_str(
            &restarted
                .handle_line(
                    &json!({
                        "type": "request", "id": "restored", "method": "warning.page",
                        "params": {"runId": run_id, "pageSize": 1}
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(restored["result"]["snapshotState"], "terminal");
        assert_eq!(restored["result"]["runStatus"], "interrupted");
        assert_eq!(restored["result"]["warningCount"], 9);
        assert_eq!(restored["result"]["accountedWarningCount"], 9);
        assert!(restored["result"]["snapshotRevision"].as_i64().unwrap() > first_revision);
    }

    #[test]
    fn startup_reconciles_an_abandoned_run_to_interrupted() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        let session = db
            .create_session("Recovery", &["/tmp".into()], &[])
            .unwrap();
        let run = db
            .create_scan_run(
                session,
                &RunParameters {
                    roots: vec!["/tmp".into()],
                    ignore_patterns: vec![],
                    directory_similarity_threshold_millis: 500,
                    cloud_policy: Default::default(),
                    manual_location_exclusions: vec![],
                    registered_cloud_locations: vec![],
                    cloud_detection_status: Default::default(),
                },
                "test",
            )
            .unwrap();
        db.start_scan_run(run).unwrap();
        drop(db);
        let frames = execute(
            &temp,
            &[
                HELLO.to_owned(),
                r#"{"type":"request","id":"get","method":"run.get","params":{"runId":1}}"#
                    .to_owned(),
            ],
        );
        assert_eq!(
            response(&frames, "get")["result"]["run"]["status"],
            "interrupted"
        );
    }
}

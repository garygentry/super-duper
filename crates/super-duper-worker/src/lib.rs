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
use super_duper_core::storage::models::{
    CloudDetectionStatus, CloudPolicy, DuplicateFileGroupFilter, DuplicateFileGroupPageQuery,
    DuplicateFileGroupResult, DuplicateFileGroupSortField, DuplicateFileMemberFilter,
    DuplicateFileMemberPageQuery, DuplicateFileMemberResult, DuplicateFileMemberSortField,
    DuplicateFileSelectedRootFacetPageQuery, DuplicateFileSelectedRootFacetResult,
    DuplicateFileSelectedRootFacetSortField, DuplicateFolderGroupFilter,
    DuplicateFolderGroupPageQuery, DuplicateFolderGroupResult, DuplicateFolderGroupSortField,
    DuplicateFolderMemberFilter, DuplicateFolderMemberPageQuery, DuplicateFolderMemberResult,
    DuplicateFolderMemberSortField, PageCursor, PageCursorValue, RegisteredCloudLocation,
    RunExclusion, RunParameters, ScanRun, ScanSession, SortDirection,
};
use super_duper_core::storage::Database;
use super_duper_core::{AppConfig, ScanEngine};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAXIMUM_FRAME_BYTES: usize = 1_048_576;
const DEFAULT_PAGE_SIZE: i64 = 100;
const DEFAULT_RESULT_PAGE_SIZE: i64 = 200;
const MAXIMUM_PAGE_SIZE: i64 = 500;
const MAXIMUM_FILTER_CHARACTERS: usize = 512;
const MAXIMUM_CURSOR_CHARACTERS: usize = MAXIMUM_FRAME_BYTES / 2;
const EVENT_INTERVAL: Duration = Duration::from_millis(100);
const DATABASE_PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

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
}

impl WorkerOptions {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }
}

impl Default for WorkerOptions {
    fn default() -> Self {
        Self::new(
            std::env::var_os("SUPER_DUPER_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("super_duper.db")),
        )
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateFileGroupFilterParameters {
    #[serde(default)]
    search: Option<String>,
    #[serde(default = "default_minimum_size")]
    minimum_size: String,
    #[serde(default)]
    across_drives: bool,
    #[serde(default)]
    selected_root: Option<String>,
}

impl Default for DuplicateFileGroupFilterParameters {
    fn default() -> Self {
        Self {
            search: None,
            minimum_size: default_minimum_size(),
            across_drives: false,
            selected_root: None,
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
    #[serde(default = "default_minimum_size")]
    minimum_size: String,
    #[serde(default)]
    across_drives: bool,
}

impl Default for DuplicateFileSelectedRootFacetFilterParameters {
    fn default() -> Self {
        Self {
            search: None,
            minimum_size: default_minimum_size(),
            across_drives: false,
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
}

struct ActiveRun {
    run_id: i64,
    cancel_token: Arc<AtomicBool>,
}

struct SharedState {
    database_path: PathBuf,
    active: Mutex<Option<ActiveRun>>,
    idle: Condvar,
    output: Sender<String>,
}

impl SharedState {
    fn new(options: WorkerOptions, output: Sender<String>) -> Result<Arc<Self>, WorkerError> {
        let database_path = options.database_path;
        Database::open(&database_path.to_string_lossy()).map_err(|error| {
            WorkerError::Startup(format!("worker database initialization failed: {error}"))
        })?;
        Ok(Arc::new(Self {
            database_path,
            active: Mutex::new(None),
            idle: Condvar::new(),
            output,
        }))
    }

    fn database(&self) -> Result<Database, ProtocolFailure> {
        Database::open_connection(&self.database_path.to_string_lossy())
            .map_err(internal_database_error)
    }

    fn active_run_id(&self) -> Option<i64> {
        self.active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|active| active.run_id)
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
            })),
            "session.list" => self.session_list(request),
            "session.get" => self.session_get(request),
            "session.create" => self.session_create(request),
            "session.update" => self.session_update(request),
            "session.delete" => self.session_delete(request),
            "run.list" => self.run_list(request),
            "run.get" => self.run_get(request),
            "run_exclusion.page" => self.run_exclusion_page(request),
            "run.start" => self.run_start(request),
            "run.cancel" => self.run_cancel(request),
            "duplicate_file_group.page" => self.duplicate_file_group_page(request),
            "duplicate_file_selected_root_facet.page" => {
                self.duplicate_file_selected_root_facet_page(request)
            }
            "duplicate_file_group.members" => self.duplicate_file_group_members(request),
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
        let search = validate_search(parameters.filter.search)?;
        let minimum_size =
            parse_non_negative_decimal(&parameters.filter.minimum_size, "filter.minimumSize")?;
        let selected_root = parameters
            .filter
            .selected_root
            .filter(|value| !value.is_empty());
        let signature = group_query_signature(
            parameters.run_id,
            sort_field,
            sort_direction,
            search.as_deref(),
            minimum_size,
            parameters.filter.across_drives,
            selected_root.as_deref(),
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
                    minimum_size,
                    across_drives: parameters.filter.across_drives,
                    selected_root,
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
        let search = validate_search(parameters.filter.search)?;
        let minimum_size =
            parse_non_negative_decimal(&parameters.filter.minimum_size, "filter.minimumSize")?;
        let signature = selected_root_facet_query_signature(
            parameters.run_id,
            sort_field,
            sort_direction,
            search.as_deref(),
            minimum_size,
            parameters.filter.across_drives,
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
                    minimum_size,
                    across_drives: parameters.filter.across_drives,
                    selected_root: None,
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
        let signature = folder_member_query_signature(
            parameters.run_id,
            parameters.group_id,
            sort_field,
            sort_direction,
            search.as_deref(),
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
        }))
    }

    fn run_start(&self, request: &RequestEnvelope) -> Result<Value, ProtocolFailure> {
        let parameters: IdParameters = parse_parameters(request)?;
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
}

struct ProgressState {
    phase: &'static str,
    files_discovered: usize,
    bytes_discovered: u64,
    files_hashed: usize,
    warning_count: usize,
    phase_warning_base: usize,
    sequence: u64,
    last_event: Option<Instant>,
    last_database_write: Option<Instant>,
}

impl WorkerProgressReporter {
    fn new(state: Arc<SharedState>, run_id: i64, cancel_token: Arc<AtomicBool>) -> Self {
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
                phase_warning_base: 0,
                sequence: 0,
                last_event: None,
                last_database_write: None,
            }),
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
                progress.phase_warning_base = progress.warning_count;
            }
        }
        self.update(Some(phase), None, true);
    }

    fn update(&self, phase: Option<&'static str>, current_path: Option<&str>, force: bool) {
        let now = Instant::now();
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(phase) = phase {
            progress.phase = phase;
        }
        let write_database = progress.last_database_write.map_or(true, |last| {
            now.duration_since(last) >= DATABASE_PROGRESS_INTERVAL
        });
        let emit_event = force
            || progress
                .last_event
                .map_or(true, |last| now.duration_since(last) >= EVENT_INTERVAL);
        if !write_database && !emit_event {
            return;
        }

        let phase = progress.phase;
        let files_discovered = progress.files_discovered;
        let bytes_discovered = progress.bytes_discovered;
        let files_hashed = progress.files_hashed;
        let warning_count = progress.warning_count;
        if write_database {
            progress.last_database_write = Some(now);
        }
        if emit_event {
            progress.last_event = Some(now);
            progress.sequence += 1;
        }
        let sequence = progress.sequence;
        drop(progress);

        if write_database {
            if let Ok(db) = Database::open_connection(&self.state.database_path.to_string_lossy()) {
                if let Err(error) = db.update_run_progress(
                    self.run_id,
                    phase,
                    files_discovered as i64,
                    bytes_discovered.min(i64::MAX as u64) as i64,
                    files_hashed as i64,
                    warning_count as i64,
                ) {
                    if !matches!(db.get_scan_run(self.run_id), Ok(run) if run.status == "completed" || run.status == "cancelled" || run.status == "failed")
                    {
                        eprintln!(
                            "worker progress persistence failed for run {}: {error}",
                            self.run_id
                        );
                    }
                }
            }
        }
        if emit_event {
            let status = if self.cancel_token.load(Ordering::Acquire) {
                "cancelling"
            } else {
                "running"
            };
            let mut data = json!({
                "runId":self.run_id,
                "sequence":sequence,
                "status":status,
                "phase":phase,
                "filesDiscovered":files_discovered,
                "bytesDiscovered":bytes_discovered.to_string(),
                "filesHashed":files_hashed,
                "warningCount":warning_count,
            });
            if let Some(path) = current_path.filter(|path| !path.is_empty()) {
                data["currentPath"] = Value::String(path.to_owned());
            }
            if warning_count > 0 {
                data["message"] = Value::String(
                    "The scan encountered recoverable warnings; see local diagnostics.".to_owned(),
                );
            }
            self.state.emit("run.progress", &data);
        }
    }
}

impl ProgressReporter for WorkerProgressReporter {
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

fn group_query_signature(
    run_id: i64,
    sort_field: DuplicateFileGroupSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
    minimum_size: i64,
    across_drives: bool,
    selected_root: Option<&str>,
) -> String {
    json!({
        "runId": run_id,
        "sortField": group_sort_name(sort_field),
        "sortDirection": direction_name(sort_direction),
        "search": search.unwrap_or_default(),
        "minimumSize": minimum_size,
        "acrossDrives": across_drives,
        "selectedRoot": selected_root.unwrap_or_default(),
    })
    .to_string()
}

fn selected_root_facet_query_signature(
    run_id: i64,
    sort_field: DuplicateFileSelectedRootFacetSortField,
    sort_direction: SortDirection,
    search: Option<&str>,
    minimum_size: i64,
    across_drives: bool,
) -> String {
    json!({
        "runId": run_id,
        "sortField": selected_root_facet_sort_name(sort_field),
        "sortDirection": direction_name(sort_direction),
        "search": search.unwrap_or_default(),
        "minimumSize": minimum_size,
        "acrossDrives": across_drives,
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
) -> String {
    format!(
        "{run_id}|{group_id}|{}|{}|{}",
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

fn selected_root_facet_sort_name(field: DuplicateFileSelectedRootFacetSortField) -> &'static str {
    match field {
        DuplicateFileSelectedRootFacetSortField::MatchingGroupCount => "matchingGroupCount",
        DuplicateFileSelectedRootFacetSortField::Value => "value",
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

fn internal_database_error(error: rusqlite::Error) -> ProtocolFailure {
    ProtocolFailure::new(
        "internal_error",
        format!("Database operation failed: {error}"),
    )
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

fn default_group_sort_field() -> String {
    "recoverableBytes".to_owned()
}

fn default_selected_root_facet_sort_field() -> String {
    "matchingGroupCount".to_owned()
}

fn default_folder_group_sort_field() -> String {
    "totalBytes".to_owned()
}

fn default_member_sort_field() -> String {
    "path".to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    const HELLO: &str = r#"{"type":"request","id":"hello-1","method":"hello","params":{"protocolVersions":[1],"client":{"name":"protocol-test","version":"1.0.0"}}}"#;

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
                canonical_path: "/root/a.txt".to_owned(),
                relative_path: "a.txt".to_owned(),
                file_name: "a.txt".to_owned(),
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
                canonical_path: "/root/a-copy.txt".to_owned(),
                relative_path: "a-copy.txt".to_owned(),
                file_name: "a-copy.txt".to_owned(),
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
        ])
        .unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[
                (
                    11,
                    100,
                    vec!["/root/a.txt".to_owned(), "/root/a-copy.txt".to_owned()],
                ),
                (
                    22,
                    200,
                    vec!["/root/b.bin".to_owned(), "/root/b-copy.bin".to_owned()],
                ),
            ],
        )
        .unwrap();
        db.complete_scan_run(run_id, 4, 600, 4, 2, 0, 300, 0)
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
        assert_eq!(first["result"]["summary"]["matchingCopyCount"], 4);
        assert_eq!(
            first["result"]["summary"]["potentialRecoverableBytes"],
            "300"
        );
        assert_eq!(first["result"]["summary"]["largestRecoverableBytes"], "200");
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
        let cursor = first["result"]["nextCursor"].as_str().unwrap();
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
        assert_eq!(members["result"]["members"].as_array().unwrap().len(), 2);
        assert!(members["result"]["members"][0]["modifiedTimeUnixNanos"].is_string());
        assert!(members["result"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|member| member["rootPath"] == "/archive"
                && member["relativePath"] == "copies/b-copy.bin"
                && member["driveLetter"] == "E:"));
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
    fn completed_run_emits_ordered_phases_and_matching_terminal_state() {
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
        let mut phase_transitions = phases;
        phase_transitions.dedup();
        assert_eq!(
            phase_transitions,
            vec![
                "discovering",
                "hashing",
                "persisting",
                "analyzing_folders",
                "finalizing"
            ]
        );
        assert!(progress.windows(2).all(|pair| {
            pair[0]["data"]["sequence"].as_u64().unwrap()
                < pair[1]["data"]["sequence"].as_u64().unwrap()
        }));
        assert!(events.iter().any(|frame| {
            frame["event"] == "run.completed" && frame["data"]["run"]["status"] == "completed"
        }));
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

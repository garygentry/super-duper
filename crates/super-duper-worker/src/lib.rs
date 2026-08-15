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
use super_duper_core::storage::models::{RunParameters, ScanRun, ScanSession};
use super_duper_core::storage::Database;
use super_duper_core::{AppConfig, ScanEngine};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAXIMUM_FRAME_BYTES: usize = 1_048_576;
const DEFAULT_PAGE_SIZE: i64 = 100;
const MAXIMUM_PAGE_SIZE: i64 = 500;
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
struct SessionWriteParameters {
    name: String,
    roots: Vec<String>,
    #[serde(default)]
    ignore_patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionUpdateParameters {
    session_id: i64,
    name: String,
    roots: Vec<String>,
    #[serde(default)]
    ignore_patterns: Vec<String>,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDto {
    id: i64,
    name: String,
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
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
    error_message: Option<String>,
    engine_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunParametersDto {
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
    directory_similarity_threshold_millis: u16,
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
            "run.start" => self.run_start(request),
            "run.cancel" => self.run_cancel(request),
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
        )?;
        let db = self.state.database()?;
        if session_name_exists(&db, &validated.name, None)? {
            return Err(ProtocolFailure::new(
                "session_name_conflict",
                "A session with that name already exists",
            ));
        }
        let id = db
            .create_session(
                &validated.name,
                &validated.roots,
                &validated.ignore_patterns,
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
        )?;
        let db = self.state.database()?;
        let _ = get_session(&db, parameters.session_id)?;
        if session_name_exists(&db, &validated.name, Some(parameters.session_id))? {
            return Err(ProtocolFailure::new(
                "session_name_conflict",
                "A session with that name already exists",
            ));
        }
        db.update_session(
            parameters.session_id,
            &validated.name,
            &validated.roots,
            &validated.ignore_patterns,
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
        if !session.roots.iter().any(|root| Path::new(root).is_dir()) {
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
        current.cancel_token.store(true, Ordering::Release);
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

    fn on_db_write_start(&self) {
        self.phase("persisting");
    }

    fn on_db_write_progress(&self, _rows: usize, _total_rows: usize) {
        self.update(None, None, false);
    }

    fn on_dir_analysis_start(&self) {
        self.phase("analyzing_folders");
    }

    fn on_dir_analysis_progress(&self, _completed: usize, _total: usize) {
        self.update(None, None, false);
    }

    fn on_finalizing(&self) {
        self.phase("finalizing");
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

struct ValidatedSession {
    name: String,
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
}

fn validate_session(
    name: String,
    roots: Vec<String>,
    ignore_patterns: Vec<String>,
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
        let canonical = fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root));
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
    })
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

fn session_dto(session: ScanSession) -> Result<SessionDto, ProtocolFailure> {
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
        error_message: run.error_message,
        engine_version: run.engine_version,
    })
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
    fn second_start_is_busy_and_cancellation_reaches_durable_cancelled_state() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("scan");
        fs::create_dir(&root).unwrap();
        for index in 0..200 {
            fs::write(root.join(format!("{index}.bin")), vec![index as u8; 4096]).unwrap();
        }
        let db_path = temp.path().join("worker.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        db.create_session("Scan", &[root.to_string_lossy().into_owned()], &[])
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
        db.create_session("Scan", &[root.to_string_lossy().into_owned()], &[])
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
        assert_eq!(
            phases,
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

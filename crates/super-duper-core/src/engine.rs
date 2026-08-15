use crate::analysis::{dir_fingerprint, dir_similarity};
use crate::config::{self, AppConfig};
use crate::error::Error;
use crate::hasher;
use crate::platform;
use crate::progress::ProgressReporter;
use crate::scanner;
use crate::storage::models::{RunParameters, ScannedFile};
use crate::storage::Database;
use dashmap::DashMap;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

pub struct ScanEngine {
    config: AppConfig,
    db_path: String,
    session_id: Option<i64>,
    cancel_token: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub session_id: i64,
    pub run_id: i64,
    pub scan_duration: Duration,
    pub hash_duration: Duration,
    pub db_write_duration: Duration,
    pub dir_analysis_duration: Duration,
    pub total_files_scanned: usize,
    pub total_bytes_discovered: u64,
    pub files_hashed: usize,
    pub duplicate_groups: usize,
    pub duplicate_files: usize,
    pub wasted_bytes: u64,
    pub warning_count: usize,
    pub dir_fingerprints: usize,
    pub dir_similarity_pairs: usize,
}

#[derive(Debug)]
pub struct ScanStats {
    pub distinct_sizes: u64,
    pub total_files: usize,
    pub total_size: u64,
}

struct PersistedResults {
    groups: usize,
    duplicate_files: usize,
    wasted_bytes: u64,
    warnings: usize,
}

impl ScanEngine {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            db_path: "super_duper.db".to_string(),
            session_id: None,
            cancel_token: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_db_path(mut self, path: &str) -> Self {
        self.db_path = path.to_string();
        self
    }

    /// Bind execution to an existing reusable session definition.
    pub fn with_session_id(mut self, session_id: i64) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn cancel(&self) {
        self.cancel_token.store(true, Ordering::Relaxed);
    }

    pub fn cancel_token(&self) -> Arc<AtomicBool> {
        self.cancel_token.clone()
    }

    /// Creates a durable run before traversal, then persists exactly one terminal outcome.
    pub fn scan(&self, progress: &dyn ProgressReporter) -> Result<ScanResult, Error> {
        self.cancel_token.store(false, Ordering::Relaxed);

        let roots = config::non_overlapping_directories(self.config.root_paths.clone());
        let db = Database::open(&self.db_path)?;
        let session_id = match self.session_id {
            Some(id) => {
                db.get_session(id)?;
                id
            }
            None => db.ensure_default_session(&roots, &self.config.ignore_patterns)?,
        };
        let parameters = RunParameters {
            roots: roots.clone(),
            ignore_patterns: self.config.ignore_patterns.clone(),
            directory_similarity_threshold_millis: 500,
        };
        let run_id = db.create_scan_run(session_id, &parameters, crate::ENGINE_VERSION)?;
        if let Err(error) = db.start_scan_run(run_id) {
            let _ = db.fail_scan_run(run_id, &error.to_string());
            return Err(error.into());
        }

        self.finish_started_run(&db, session_id, run_id, &parameters, progress)
    }

    /// Executes a run that a coordinator has already transitioned to `running`.
    ///
    /// Unlike [`scan`](Self::scan), this does not reset the cancellation token. That lets a
    /// process coordinator publish the run ID and accept cancellation before the scan thread has
    /// entered traversal.
    pub fn execute_started_run(
        &self,
        run_id: i64,
        progress: &dyn ProgressReporter,
    ) -> Result<ScanResult, Error> {
        let db = Database::open_connection(&self.db_path)?;
        let run = db.get_scan_run(run_id)?;
        if run.status == "cancelling" && self.cancel_token.load(Ordering::Acquire) {
            db.cancel_scan_run(run_id)?;
            return Err(Error::Cancelled);
        }
        if run.status != "running" {
            return Err(Error::Other(format!(
                "run {run_id} is not in the running state"
            )));
        }
        let parameters = RunParameters::from_json(&run.parameters_json)
            .ok_or_else(|| Error::Other(format!("run {run_id} has invalid parameters")))?;
        self.finish_started_run(&db, run.session_id, run_id, &parameters, progress)
    }

    fn finish_started_run(
        &self,
        db: &Database,
        session_id: i64,
        run_id: i64,
        parameters: &RunParameters,
        progress: &dyn ProgressReporter,
    ) -> Result<ScanResult, Error> {
        let roots = &parameters.roots;

        info!(
            "Processing run {} for session {}: {:?}",
            run_id, session_id, roots
        );
        let result = self.execute_run(
            db,
            session_id,
            run_id,
            roots,
            &parameters.ignore_patterns,
            progress,
        );
        match result {
            Ok(result) => Ok(result),
            Err(Error::Cancelled) => {
                let _ = db.mark_run_cancelling(run_id);
                db.cancel_scan_run(run_id)?;
                Err(Error::Cancelled)
            }
            Err(_) if self.cancel_token.load(Ordering::Acquire) => {
                let _ = db.mark_run_cancelling(run_id);
                db.cancel_scan_run(run_id)?;
                Err(Error::Cancelled)
            }
            Err(error) => {
                db.fail_scan_run(run_id, &error.to_string())?;
                Err(error)
            }
        }
    }

    fn execute_run(
        &self,
        db: &Database,
        session_id: i64,
        run_id: i64,
        roots: &[String],
        ignore_patterns: &[String],
        progress: &dyn ProgressReporter,
    ) -> Result<ScanResult, Error> {
        let root_slices: Vec<&str> = roots.iter().map(String::as_str).collect();
        let ignore_slices: Vec<&str> = ignore_patterns.iter().map(String::as_str).collect();

        progress.on_scan_start();
        let scan_start = Instant::now();
        let traversal =
            scanner::discover_files(&root_slices, &ignore_slices, &self.cancel_token, progress)?;
        let scan_duration = scan_start.elapsed();
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let stats = compute_scan_stats(&traversal.size_to_files);
        debug_assert_eq!(stats.total_files, traversal.files_discovered);
        debug_assert_eq!(stats.total_size, traversal.bytes_discovered);
        progress.on_scan_complete(traversal.files_discovered, scan_duration.as_secs_f64());
        db.update_run_progress(
            run_id,
            "hashing",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            0,
            traversal.warning_count as i64,
        )?;
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        progress.on_hash_start();
        let hash_start = Instant::now();
        let hash_outcome = match hasher::build_content_hash_map_with_stats(
            traversal.size_to_files,
            &self.cancel_token,
            progress,
        ) {
            Ok(outcome) => outcome,
            Err(_) if self.cancel_token.load(Ordering::Relaxed) => return Err(Error::Cancelled),
            Err(error) => return Err(error.into()),
        };
        let hash_duration = hash_start.elapsed();
        let warning_count = traversal.warning_count + hash_outcome.warning_count;
        progress.on_hash_complete(
            hash_outcome.confirmed_duplicates.len(),
            hash_duration.as_secs_f64(),
        );
        db.update_run_progress(
            run_id,
            "persisting",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            warning_count as i64,
        )?;
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        progress.on_db_write_start();
        let db_start = Instant::now();
        let persisted = persist_run_results(
            db,
            run_id,
            traversal.files,
            &hash_outcome.confirmed_duplicates,
            &self.cancel_token,
            progress,
        )?;
        let db_duration = db_start.elapsed();
        let warning_count = warning_count + persisted.warnings;
        progress.on_db_write_complete(traversal.files_discovered, db_duration.as_secs_f64());
        db.update_run_progress(
            run_id,
            "analyzing_folders",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            warning_count as i64,
        )?;
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        progress.on_dir_analysis_start();
        let dir_start = Instant::now();
        let dir_fingerprints = dir_fingerprint::build_directory_fingerprints_cancellable(
            db,
            run_id,
            &self.cancel_token,
            progress,
        )?;
        let dir_similarity_pairs = dir_similarity::compute_directory_similarity_cancellable(
            db,
            run_id,
            0.5,
            &self.cancel_token,
            progress,
        )?;
        let dir_duration = dir_start.elapsed();
        progress.on_dir_analysis_complete(
            dir_fingerprints,
            dir_similarity_pairs,
            dir_duration.as_secs_f64(),
        );
        db.update_run_progress(
            run_id,
            "finalizing",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            warning_count as i64,
        )?;
        progress.on_finalizing();
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        db.complete_scan_run(
            run_id,
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            persisted.groups as i64,
            0,
            persisted.wasted_bytes as i64,
            warning_count as i64,
        )?;

        Ok(ScanResult {
            session_id,
            run_id,
            scan_duration,
            hash_duration,
            db_write_duration: db_duration,
            dir_analysis_duration: dir_duration,
            total_files_scanned: traversal.files_discovered,
            total_bytes_discovered: traversal.bytes_discovered,
            files_hashed: hash_outcome.files_hashed,
            duplicate_groups: persisted.groups,
            duplicate_files: persisted.duplicate_files,
            wasted_bytes: persisted.wasted_bytes,
            warning_count,
            dir_fingerprints,
            dir_similarity_pairs,
        })
    }
}

fn compute_scan_stats(map: &DashMap<u64, Vec<PathBuf>>) -> ScanStats {
    let mut stats = ScanStats {
        distinct_sizes: 0,
        total_files: 0,
        total_size: 0,
    };
    for entry in map {
        stats.distinct_sizes += 1;
        stats.total_files += entry.value().len();
        stats.total_size += entry.key() * entry.value().len() as u64;
    }
    stats
}

fn persist_run_results(
    db: &Database,
    run_id: i64,
    discovered: Vec<scanner::DiscoveredFile>,
    content_hash_map: &DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> Result<PersistedResults, Error> {
    let mut hashes_by_path = HashMap::new();
    let mut groups = Vec::new();
    let mut warnings = 0;
    let mut wasted_bytes = 0u64;
    let mut duplicate_files = 0usize;

    for entry in content_hash_map {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let hash = *entry.key() as i64;
        let mut paths = Vec::new();
        let mut file_size = 0i64;
        for path in entry.value() {
            if cancel_token.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let canonicalized = fs::canonicalize(path).and_then(|canonical| {
                file_size = fs::metadata(&canonical)?.len() as i64;
                Ok(canonical.to_string_lossy().into_owned())
            });
            match canonicalized {
                Ok(path) => {
                    hashes_by_path.insert(path.clone(), hash);
                    paths.push(path);
                }
                Err(error) => {
                    warnings += 1;
                    tracing::warn!(
                        "Duplicate candidate disappeared before persistence: {}",
                        error
                    );
                }
            }
        }
        if paths.len() > 1 {
            duplicate_files += paths.len();
            wasted_bytes += file_size as u64 * (paths.len() as u64 - 1);
            groups.push((hash, file_size, paths));
        }
    }

    let total_files = discovered.len();
    let mut files = Vec::with_capacity(total_files);
    for file in discovered {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let canonical = PathBuf::from(&file.canonical_path);
        files.push(ScannedFile {
            id: 0,
            run_id,
            root_path: file.root_path,
            relative_path: file.relative_path,
            file_name: file.file_name,
            parent_dir: file.parent_dir,
            drive_letter: platform::get_drive_letter(&canonical)
                .map(|drive| drive.to_string_lossy().into_owned())
                .unwrap_or_default(),
            file_size: file.file_size as i64,
            last_modified: file.last_modified,
            partial_hash: None,
            content_hash: hashes_by_path.get(&file.canonical_path).copied(),
            file_identity: None,
            warning_message: file.warning_message,
            marked_deleted: false,
            canonical_path: file.canonical_path,
        });
    }

    let mut persisted_rows = 0;
    for batch in files.chunks(256) {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        persisted_rows += db.insert_scanned_files(batch)?;
        progress.on_db_write_progress(persisted_rows, total_files);
    }

    let mut group_count = 0;
    for batch in groups.chunks(64) {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        group_count += db.insert_duplicate_groups_cancellable(run_id, batch, cancel_token)?;
    }
    Ok(PersistedResults {
        groups: group_count,
        duplicate_files,
        wasted_bytes,
        warnings,
    })
}

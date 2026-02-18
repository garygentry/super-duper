use crate::analysis::{dir_fingerprint, dir_similarity};
use crate::config::{self, AppConfig};
use crate::error::Error;
use crate::hasher;
use crate::platform;
use crate::progress::ProgressReporter;
use crate::scanner;
use crate::storage::models::ScannedFile;
use crate::storage::Database;
use dashmap::DashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tracing::{debug, info};

pub struct ScanEngine {
    config: AppConfig,
    db_path: String,
    cancel_token: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub session_id: i64,
    pub scan_duration: Duration,
    pub hash_duration: Duration,
    pub db_write_duration: Duration,
    pub dir_analysis_duration: Duration,
    pub total_files_scanned: usize,
    pub duplicate_groups: usize,
    pub duplicate_files: usize,
    pub wasted_bytes: u64,
    pub dir_fingerprints: usize,
    pub dir_similarity_pairs: usize,
}

#[derive(Debug)]
pub struct ScanStats {
    pub distinct_sizes: u64,
    pub total_files: usize,
    pub total_size: u64,
}

impl ScanEngine {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            db_path: "super_duper.db".to_string(),
            cancel_token: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_db_path(mut self, path: &str) -> Self {
        self.db_path = path.to_string();
        self
    }

    /// Request cancellation of the current scan.
    pub fn cancel(&self) {
        self.cancel_token.store(true, Ordering::Relaxed);
    }

    /// Get a clone of the cancel token (for FFI layer to store).
    pub fn cancel_token(&self) -> Arc<AtomicBool> {
        self.cancel_token.clone()
    }

    /// Run the full duplicate detection pipeline:
    /// 1. Parallel directory scan (build file_size → paths map)
    /// 2. Two-tier content hashing (partial 1KB, then full on matches)
    /// 3. Write results to SQLite
    pub fn scan(&self, progress: &dyn ProgressReporter) -> Result<ScanResult, Error> {
        // Reset cancel token for new scan
        self.cancel_token.store(false, Ordering::Relaxed);

        let non_overlapping =
            config::non_overlapping_directories(self.config.root_paths.clone());
        info!("Processing directories: {:?}", non_overlapping);

        let root_path_slices: Vec<&str> = non_overlapping.iter().map(|s| s.as_str()).collect();
        let ignore_pattern_slices: Vec<&str> =
            self.config.ignore_patterns.iter().map(|s| s.as_str()).collect();

        // Phase 1: Scan
        info!("Scanning files...");
        progress.on_scan_start();
        let scan_start = Instant::now();
        let size_to_files_map = scanner::build_size_to_files_map(
            &root_path_slices,
            &ignore_pattern_slices,
            &self.cancel_token,
            progress,
        )?;
        let scan_duration = scan_start.elapsed();

        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        let stats = compute_scan_stats(&size_to_files_map);
        progress.on_scan_complete(stats.total_files, scan_duration.as_secs_f64());
        debug!(
            "Scan completed in {:.2}s — {} distinct sizes, {} files, {} bytes total",
            scan_duration.as_secs_f64(),
            stats.distinct_sizes,
            stats.total_files,
            stats.total_size,
        );

        // Phase 2: Hash
        info!("Building content hash for possible dupes...");
        progress.on_hash_start();
        let hash_start = Instant::now();
        let content_hash_map =
            hasher::build_content_hash_map(size_to_files_map, &self.cancel_token, progress)?;
        let hash_duration = hash_start.elapsed();

        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        let dupe_group_count = content_hash_map.len();
        progress.on_hash_complete(dupe_group_count, hash_duration.as_secs_f64());
        debug!(
            "Hash completed in {:.2}s — {} duplicate groups",
            hash_duration.as_secs_f64(),
            dupe_group_count,
        );

        // Phase 3: Write to SQLite
        info!("Writing to database...");
        progress.on_db_write_start();
        let db_start = Instant::now();
        let db = Database::open(&self.db_path)?;
        let (groups_written, files_written, wasted_bytes, session_id) =
            write_to_database(&db, &content_hash_map, &non_overlapping)?;
        let db_duration = db_start.elapsed();
        progress.on_db_write_complete(files_written, db_duration.as_secs_f64());
        debug!(
            "Database write completed in {:.2}s — {} groups, {} files (session {})",
            db_duration.as_secs_f64(),
            groups_written,
            files_written,
            session_id,
        );

        // Phase 4: Directory fingerprints + similarity
        info!("Analyzing directory structure...");
        progress.on_dir_analysis_start();
        let dir_start = Instant::now();
        let dir_fingerprints = dir_fingerprint::build_directory_fingerprints(&db)
            .unwrap_or_else(|e| { tracing::warn!("Directory fingerprint failed: {}", e); 0 });
        let dir_similarity_pairs = dir_similarity::compute_directory_similarity(&db, 0.5)
            .unwrap_or_else(|e| { tracing::warn!("Directory similarity failed: {}", e); 0 });
        let dir_duration = dir_start.elapsed();
        progress.on_dir_analysis_complete(dir_fingerprints, dir_similarity_pairs, dir_duration.as_secs_f64());
        debug!(
            "Directory analysis completed in {:.2}s — {} fingerprints, {} similar pairs",
            dir_duration.as_secs_f64(),
            dir_fingerprints,
            dir_similarity_pairs,
        );

        Ok(ScanResult {
            session_id,
            scan_duration,
            hash_duration,
            db_write_duration: db_duration,
            dir_analysis_duration: dir_duration,
            total_files_scanned: stats.total_files,
            duplicate_groups: groups_written,
            duplicate_files: files_written,
            wasted_bytes,
            dir_fingerprints,
            dir_similarity_pairs,
        })
    }
}

fn compute_scan_stats(map: &DashMap<u64, Vec<PathBuf>>) -> ScanStats {
    let mut distinct_sizes = 0u64;
    let mut total_files = 0usize;
    let mut total_size = 0u64;

    for entry in map.iter() {
        distinct_sizes += 1;
        total_files += entry.value().len();
        total_size += entry.key() * entry.value().len() as u64;
    }

    ScanStats {
        distinct_sizes,
        total_files,
        total_size,
    }
}

fn write_to_database(
    db: &Database,
    content_hash_map: &DashMap<u64, Vec<PathBuf>>,
    root_paths: &[String],
) -> Result<(usize, usize, u64, i64), Error> {
    // Find or create session (idempotent: reuses existing session for same paths)
    let session_id = db.find_or_create_session(root_paths)?;

    // Build file records and duplicate group info
    let mut all_files: Vec<ScannedFile> = Vec::new();
    let mut dupe_groups: Vec<(i64, i64, Vec<String>)> = Vec::new();
    let mut total_wasted: u64 = 0;

    for entry in content_hash_map.iter() {
        let content_hash = *entry.key();
        let paths = entry.value();

        let mut group_paths: Vec<String> = Vec::new();
        let mut file_size_for_group: i64 = 0;

        for path in paths.iter() {
            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Error reading metadata for {}: {}", path.display(), e);
                    continue;
                }
            };

            let canonical_path = match fs::canonicalize(path) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Error canonicalizing {}: {}", path.display(), e);
                    continue;
                }
            };

            let canonical_str = canonical_path.to_string_lossy().into_owned();

            let drive_letter = match platform::get_drive_letter(&canonical_path) {
                Some(drive) => drive.to_string_lossy().into_owned(),
                None => String::new(),
            };

            let parent_dir = canonical_path
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();

            let file_name = canonical_path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();

            let last_modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let file_size = metadata.len() as i64;
            file_size_for_group = file_size;

            group_paths.push(canonical_str.clone());

            all_files.push(ScannedFile {
                id: 0,
                canonical_path: canonical_str,
                file_name,
                parent_dir,
                drive_letter,
                file_size,
                last_modified,
                partial_hash: None,
                content_hash: Some(content_hash as i64),
                last_seen_session_id: Some(session_id),
                marked_deleted: false,
            });
        }

        if group_paths.len() > 1 {
            let wasted = file_size_for_group as u64 * (group_paths.len() as u64 - 1);
            total_wasted += wasted;
            dupe_groups.push((content_hash as i64, file_size_for_group, group_paths));
        }
    }

    // Upsert files into the global file index
    let files_written = db.insert_scanned_files(&all_files)?;

    // Insert duplicate groups for this session (old groups were pre-deleted by find_or_create_session)
    let groups_written = db.insert_duplicate_groups(session_id, &dupe_groups)?;

    // Complete session
    let total_bytes: i64 = all_files.iter().map(|f| f.file_size).sum();
    db.complete_scan_session(session_id, files_written as i64, total_bytes)?;

    Ok((groups_written, files_written, total_wasted, session_id))
}

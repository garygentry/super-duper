use super::schema;
use super::sd_pg::*;
use crate::file_proc::status::StatusMessage;
use diesel::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use crate::file_proc::stats::FileProcStats;
use super::dupe_file::DupeFile;

pub struct DupeFileDb {}

#[derive(Debug, Insertable)]
#[diesel(table_name = schema::dedupe_session)]
pub struct DedupeSession {
    // pub id: i32,
    pub run_start_time: SystemTime,
    pub process_duration: f64,
    pub scan_duration: f64,
    pub scan_input_paths: String,
    pub scan_file_count: i32,
    pub scan_file_size: i64,
    pub scan_size_dupe_file_count: i32,
    pub scan_size_dupe_file_size: i64,
    pub hash_duration: f64,
    pub hash_scan_file_count: i32,
    pub hash_scan_file_size: i64,
    pub hash_cache_hit_full_count: i32,
    pub hash_cache_hit_partial_count: i32,
    pub hash_gen_partial_count: i32,
    pub hash_gen_partial_duration: f64,
    pub hash_gen_partial_file_size: i64,
    pub hash_gen_full_count: i32,
    pub hash_gen_full_file_size: i64,
    pub hash_gen_full_duration: f64,
    pub hash_confirmed_dupe_count: i32,
    pub hash_confirmed_dupe_size: i64,
    pub hash_confirmed_dupe_distinct_count: i32,
    pub hash_confirmed_dupe_distinct_size: i64,
    pub cache_map_to_dupe_vec_duration: f64,
    pub cache_map_to_dupe_vec_count: i32,
    pub db_dupe_file_insert_duration: f64,
    pub db_dupe_file_insert_count: i32,
}

impl DedupeSession {
    fn duration_to_secs(duration: Duration) -> f64 {
        let secs = duration.as_secs() as f64;
        let subsecs = (duration.subsec_nanos() as f64) / 1_000_000_000.0;
        secs + subsecs
    }

    fn get_elapsed(start: &Option<Instant>, end: &Option<Instant>) -> f64 {
        match (start, end) {
            (Some(start), Some(end)) => {
                let duration = end.duration_since(*start);
                DedupeSession::duration_to_secs(duration)
            }
            _ => 0.0,
        }
    }
    fn get_system_time(time: &Option<SystemTime>) -> SystemTime {
        match time {
            Some(time) => { *time }
            _ => {
                panic!("get_system_time called with None value for SystemTime");
            }
        }
    }

    pub fn from_file_proc_stats(stats: Arc<Mutex<FileProcStats>>) -> DedupeSession {
        let stats = stats.lock().unwrap();
        DedupeSession {
            run_start_time: DedupeSession::get_system_time(&stats.run_start_time),
            process_duration: DedupeSession::get_elapsed(
                &stats.process_start,
                &stats.process_finish
            ),
            scan_duration: DedupeSession::get_elapsed(&stats.scan_start, &stats.scan_finish),
            scan_input_paths: stats.scan_input_paths.join(", "),
            scan_file_count: stats.scan_file_count as i32,
            scan_file_size: stats.scan_file_size as i64,
            scan_size_dupe_file_count: stats.scan_size_dupe_file_count as i32,
            scan_size_dupe_file_size: stats.scan_size_dupe_file_size as i64,
            hash_duration: DedupeSession::get_elapsed(&stats.hash_start, &stats.hash_finish),
            hash_scan_file_count: stats.hash_scan_file_count as i32,
            hash_scan_file_size: stats.hash_scan_file_size as i64,
            hash_cache_hit_full_count: stats.hash_cache_hit_full_count as i32,
            hash_cache_hit_partial_count: stats.hash_cache_hit_partial_count as i32,
            hash_gen_partial_count: stats.hash_gen_partial_count as i32,
            hash_gen_partial_duration: DedupeSession::duration_to_secs(
                stats.hash_gen_partial_duration
            ),
            hash_gen_partial_file_size: stats.hash_gen_partial_file_size as i64,
            hash_gen_full_count: stats.hash_gen_full_count as i32,
            hash_gen_full_file_size: stats.hash_gen_full_file_size as i64,
            hash_gen_full_duration: DedupeSession::duration_to_secs(stats.hash_gen_full_duration),
            hash_confirmed_dupe_count: stats.hash_confirmed_dupe_count as i32,
            hash_confirmed_dupe_size: stats.hash_confirmed_dupe_size as i64,
            hash_confirmed_dupe_distinct_count: stats.hash_confirmed_dupe_distinct_count as i32,
            hash_confirmed_dupe_distinct_size: stats.hash_confirmed_dupe_distinct_size as i64,
            cache_map_to_dupe_vec_duration: DedupeSession::get_elapsed(
                &stats.cache_map_to_dupe_vec_start,
                &stats.cache_map_to_dupe_vec_finish
            ),
            cache_map_to_dupe_vec_count: stats.cache_map_to_dupe_vec_count as i32,
            db_dupe_file_insert_duration: DedupeSession::get_elapsed(
                &stats.db_dupe_file_insert_start,
                &stats.db_dupe_file_insert_finish
            ),
            db_dupe_file_insert_count: stats.db_dupe_file_insert_count as i32,
        }
    }
}

impl DupeFileDb {
    fn insert_session(
        connection: &mut PgConnection,
        stats: Arc<Mutex<FileProcStats>>
    ) -> Result<i32, diesel::result::Error> {
        // let mut connection = establish_connection();

        let session = DedupeSession::from_file_proc_stats(stats);

        let x = diesel
            ::insert_into(schema::dedupe_session::table)
            .values(session)
            .returning(schema::dedupe_session::id)
            .get_result(connection);
        x
    }

    pub fn write_session(
        stats: Arc<Mutex<FileProcStats>>,
        dupe_files: &[DupeFile],
        tx_status: &Arc<dyn Fn(StatusMessage) + Send + Sync>
    ) -> Result<i32, diesel::result::Error> {
        // tx_status(StatusMessage::DbDupeFileInsertStart);
        let mut connection = establish_connection();

        let session_id = DupeFileDb::insert_session(&mut connection, stats)?;

        Ok(session_id)
    }
}

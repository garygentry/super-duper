use std::fs::File;
use std::io::{ self, Write };
use std::time::{ Instant, SystemTime, UNIX_EPOCH, Duration };
// use chrono::Duration as ChronoDuration;
use indicatif::{ HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle };

#[derive(Debug, Default, Clone, Copy)]
pub struct FileProcStats {
    /// The time the process started.
    pub process_start: Option<Instant>,
    /// The time the process finished.
    pub process_finish: Option<Instant>,

    /// The time the file scan started.
    pub scan_start: Option<Instant>,
    /// The time the file scan finished.
    pub scan_finish: Option<Instant>,
    // The number of files matching input specification.
    pub scan_file_count: usize,
    // Total size of all files matching input specification.
    pub scan_file_size: u64,
    // Total number of files that have at least 1 matching file size
    pub scan_size_dupe_file_count: usize,
    // Total size of all files that have at least 1 matching file size
    pub scan_size_dupe_file_size: u64,

    // The time the hash process started.
    pub hash_start: Option<Instant>,
    // The time the hash process finished.
    pub hash_finish: Option<Instant>,
    // Total number of input files processed in Hash process
    pub hash_scan_file_count: usize,
    // Total size of all input files processed in Hash process
    pub hash_scan_file_size: u64,
    // Total number of files processed where the full hash was found in cache
    pub hash_cache_hit_full_count: usize,
    // Total number of files processed where the partial hash was found in cache
    pub hash_cache_hit_partial_count: usize,
    // Total number of partial hashes generated
    pub hash_gen_partial_count: usize,
    // Total duration of partial hashes generated
    pub hash_gen_partial_duration: Duration,
    // Total size of all partial hashes generated
    pub hash_gen_partial_file_size: u64,
    // Total number of full hashes generated
    pub hash_gen_full_count: usize,
    // Total size of all full hashes generated
    pub hash_gen_full_file_size: u64,
    // Total duration of full hashes generated
    pub hash_gen_full_duration: Duration,
    // Total number of confirmed duplicate files
    pub hash_confirmed_dupe_count: usize,
    // Total aggregate size of confirmed duplicate files
    pub hash_confirmed_dupe_size: u64,
    // Total number of distinct files that have duplicates
    pub hash_confirmed_dupe_distinct_count: usize,
    // Total aggregate size of confirmed duplicate files
    pub hash_confirmed_dupe_distinct_size: u64,

    // The time the cache to dupe process started.
    pub cache_map_to_dupe_vec_start: Option<Instant>,
    // The time the cache to dupe process finished.
    pub cache_map_to_dupe_vec_finish: Option<Instant>,
    // Total number of files processed in cache to dupe process
    pub cache_map_to_dupe_vec_count: usize,

    // The time the dupe_file database insert  process started.
    pub db_dupe_file_insert_start: Option<Instant>,
    // The time the dupe_file database insert  process finished.
    pub db_dupe_file_insert_finish: Option<Instant>,

    // Total number of rows inserted into the dupe_file database table
    pub db_dupe_file_insert_count: usize,
}

fn format_elapsed_human(start: &Option<Instant>, end: &Option<Instant>) -> String {
    match (start, end) {
        (Some(start), Some(end)) => {
            let duration = end.duration_since(*start);
            HumanDuration(duration).to_string()
            // format!("{}.{}", duration.as_secs(), duration.subsec_nanos())
        }
        _ => "[N/A]".to_string(),
    }
}

fn get_elapsed(start: &Option<Instant>, end: &Option<Instant>) -> Duration {
    match (start, end) {
        (Some(start), Some(end)) => { end.duration_since(*start) }
        _ => Duration::new(0, 0),
    }
}

fn format_elapsed_data(start: &Option<Instant>, end: &Option<Instant>) -> String {
    match (start, end) {
        (Some(start), Some(end)) => {
            let duration = end.duration_since(*start);
            let total_seconds = duration.as_secs();
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            let millis = duration.subsec_millis();
            format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
        }
        _ => "[N/A]".to_string(),
    }
}

fn format_count_human(count: usize) -> String {
    HumanCount(count as u64).to_string()
}

fn format_count_data(count: usize) -> String {
    count.to_string()
}

#[derive(Debug, Clone)]
pub enum FileProcStatsItemValueType {
    Duration(Duration),
    Count(usize),
    FileSize(u64),
}

#[derive(Debug, Clone)]
pub struct FileProcStatsItem {
    pub name: String,
    pub human_name: String,
    pub value: FileProcStatsItemValueType,
    pub human_value: String,
    pub raw_string_value: String,
}

impl FileProcStatsItem {
    pub fn new(name: &str, human_name: Option<&str>, value: FileProcStatsItemValueType) -> Self {
        let human_name = match human_name {
            Some(name) => name.to_string(),
            None =>
                name
                    .split('_')
                    .map(|s| {
                        let mut chars = s.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(" "),
        };

        let human_value = match &value {
            FileProcStatsItemValueType::Duration(duration) => {
                HumanDuration(*duration).to_string()
            }
            FileProcStatsItemValueType::Count(count) => { HumanCount(*count as u64).to_string() }
            FileProcStatsItemValueType::FileSize(size) => { HumanBytes(*size).to_string() }
        };

        let raw_string_value = match &value {
            FileProcStatsItemValueType::Duration(duration) => {
                let total_seconds = duration.as_secs();
                let hours = total_seconds / 3600;
                let minutes = (total_seconds % 3600) / 60;
                let seconds = total_seconds % 60;
                let millis = duration.subsec_millis();
                format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
            }
            FileProcStatsItemValueType::Count(count) => { count.to_string() }
            FileProcStatsItemValueType::FileSize(size) => { size.to_string() }
        };

        Self {
            name: name.to_string(),
            human_name,
            value,
            human_value,
            raw_string_value,
        }
    }
}

impl FileProcStats {
    pub fn test(&self) {
        // let elapsed_formatter = format_elapsed_human;

        // println!("Proc1: {}", elapsed_formatter(&self.process_start, &self.process_finish));
        // println!("Proc1: {}", format_elapsed_data(&self.process_start, &self.process_finish));
        let items = self.to_print_vals();
        for item in items.iter() {
            // println!("{}: {}", item.name, item.human_value);
            println!("{:?}", item);
        }
    }

    pub fn to_print_vals(self) -> Vec<FileProcStatsItem> {
        let items = vec![
            FileProcStatsItem::new(
                "process_start",
                None,
                FileProcStatsItemValueType::Duration(
                    get_elapsed(&self.process_start, &self.process_finish)
                )
            )
        ];

        items
    }

    // fn to_csv(&self, filename: &str) -> io::Result<()> {
    //     let mut file = File::create(filename)?;
    //     writeln!(
    //         file,
    //         "process_start,process_finish,scan_start,scan_finish,scan_file_count,scan_file_size,scan_size_dupe_file_count,scan_size_dupe_file_size,hash_start,hash_finish,hash_scan_file_count,hash_scan_file_size,hash_cache_hit_full_count,hash_cache_hit_partial_count,hash_gen_partial_count,hash_gen_partial_duration,hash_gen_partial_file_size,hash_gen_full_count,hash_gen_full_file_size,hash_gen_full_duration,hash_confirmed_dupe_count,hash_confirmed_dupe_size,hash_confirmed_dupe_distinct_count,hash_confirmed_dupe_distinct_size,cache_map_to_dupe_vec_start,cache_map_to_dupe_vec_finish,cache_map_to_dupe_vec_count,db_dupe_file_insert_start,db_dupe_file_insert_finish,db_dupe_file_insert_count"
    //     )?;
    //     writeln!(
    //         file,
    //         "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
    //         self.process_start.map(|t| t.to_string()).unwrap_or_else(|| String::from("")),
    //         self.process_finish.map(|t| t.to_string()).unwrap_or_else(|| String::from("")),
    //         self.scan_start.map(|t| t.to_string()).unwrap_or_else(|| String::from("")),
    //         self.scan_finish.map(|t| t.to_string()).unwrap_or_else(|| String::from("")),
    //         self.scan_file_count,
    //         self.scan_file_size,
    //         self.scan_size_dupe_file_count,
    //         self.scan_size_dupe_file_size,
    //         self.hash_start.map(|t| t.to_string()).unwrap_or_else(|| String::from("")),
    //         self.hash_finish.map(|t| t.to_string()).unwrap_or_else(|| String::from("")),
    //         self.hash_scan_file_count,
    //         self.hash_scan_file_size,
    //         self.hash_cache_hit_full_count,
    //         self.hash_cache_hit_partial_count,
    //         self.hash_gen_partial_count,
    //         self.hash_gen_partial_duration.as_secs(),
    //         self.hash_gen_partial_file_size,
    //         self.hash_gen_full_count,
    //         self.hash_gen_full_file_size,
    //         self.hash_gen_full_duration.as_secs(),
    //         self.hash_confirmed_dupe_count,
    //         self.hash_confirmed_dupe_size,
    //         self.hash_confirmed_dupe_distinct_count,
    //         self.hash_confirmed_dupe_distinct_size,
    //         self.cache_map_to_dupe_vec_start
    //             .map(|t| t.to_string())
    //             .unwrap_or_else(|| String::from("")),
    //         self.cache_map_to_dupe_vec_finish
    //             .map(|t| t.to_string())
    //             .unwrap_or_else(|| String::from("")),
    //         self.cache_map_to_dupe_vec_count,
    //         self.db_dupe_file_insert_start
    //             .map(|t| t.to_string())
    //             .unwrap_or_else(|| String::from("")),
    //         self.db_dupe_file_insert_finish
    //             .map(|t| t.to_string())
    //             .unwrap_or_else(|| String::from("")),
    //         self.db_dupe_file_insert_count
    //     )?;
    //     Ok(())
    // }
}

// fn write_stats_to_csv(stats: &FileProcStats, file_path: &str) -> csv::Result<()> {
//     let mut wtr = Writer::from_path(file_path)?;

//     // Optionally, handle Instant and Duration formatting
//     fn format_instant(instant: &Option<Instant>) -> String {
//         instant
//             .map(|inst| {
//                 let datetime: DateTime<Utc> = DateTime::<Utc>::from(inst);
//                 datetime.to_rfc3339()
//             })
//             .unwrap_or_else(|| "N/A".to_string())
//     }

//     // Write each field to the CSV
//     wtr.write_record(&["Field", "Value"])?;
//     wtr.write_record(&["process_start", &format_instant(&stats.process_start)])?;
//     wtr.write_record(&["process_finish", &format_instant(&stats.process_finish)])?;
//     wtr.write_record(&["scan_start", &format_instant(&stats.scan_start)])?;
//     wtr.write_record(&["scan_finish", &format_instant(&stats.scan_finish)])?;
//     wtr.write_record(&["scan_file_count", &stats.scan_file_count.to_string()])?;
//     wtr.write_record(&["scan_file_size", &stats.scan_file_size.to_string()])?;
//     // Add other fields similarly...

//     wtr.flush()?;
//     Ok(())
// }

use std::{ fmt, fs::{ self, OpenOptions }, time::{ Duration, Instant, SystemTime } };
use indicatif::{ HumanBytes, HumanCount, HumanDuration };
use tabled::{ Tabled, Table };
use tabled::settings::Style;
use chrono::{ DateTime, Utc };

#[derive(Debug, Default, Clone)]
pub struct StatsTimer {
    start_time: Option<Instant>,
    finish_time: Option<Instant>,
    duration: Duration,
}

impl StatsTimer {
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            finish_time: None,
            duration: Duration::new(0, 0),
        }
    }

    pub fn finish(&mut self) {
        self.finish_time = Some(Instant::now());
        self.duration = self.finish_time.unwrap().duration_since(self.start_time.unwrap());
    }

    pub fn get_duration(&self) -> Duration {
        self.duration
    }

    pub fn get_duration_secs(&self) -> f32 {
        let secs = self.duration.as_secs() as f32;
        let subsecs = (self.duration.subsec_nanos() as f32) / 1_000_000_000.0;
        secs + subsecs
    }

    pub fn get_duration_human(&self) -> String {
        HumanDuration(self.duration).to_string()
    }

    pub fn get_duration_string(&self) -> String {
        let total_seconds = self.duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let millis = self.duration.subsec_millis();
        format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
    }
}

#[derive(Debug, Default, Clone)]
pub struct FileProcStats {
    pub process_timer: StatsTimer,
    pub scan_timer: StatsTimer,
    pub hash_timer: StatsTimer,

    /// The time the process started
    pub run_start_time: Option<SystemTime>,
    /// The time the process started.
    pub process_start: Option<Instant>,
    /// The time the process finished.
    pub process_finish: Option<Instant>,

    /// The time the file scan started.
    pub scan_start: Option<Instant>,
    /// The time the file scan finished.
    pub scan_finish: Option<Instant>,
    /// The input paths to scan.
    pub scan_input_paths: Vec<String>,
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

#[derive(Debug, Clone)]
pub enum FileProcStatsValueType {
    Duration(Duration),
    Count(usize),
    FileSize(u64),
    SystemTime(SystemTime),
    StringVec(Vec<String>),
}

impl fmt::Display for FileProcStatsValueType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileProcStatsValueType::Duration(duration) => write!(f, "{}", HumanDuration(*duration)),
            FileProcStatsValueType::Count(count) => write!(f, "{}", count),
            FileProcStatsValueType::FileSize(size) => write!(f, "{}", size),
            FileProcStatsValueType::SystemTime(time) => {
                let datetime = DateTime::<Utc>::from(*time);
                let datetime = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
                write!(f, "{}", datetime)
            }
            FileProcStatsValueType::StringVec(vec) => {
                let vec_str = vec.join(", ");
                write!(f, "{}", vec_str)
            }
        }
    }
}

#[derive(Debug, Clone, Tabled)]
pub struct FileProcStatsPrintItem {
    #[tabled(skip)]
    pub name: String,
    #[tabled(rename = "Stat")]
    pub human_name: String,
    #[tabled(skip)]
    pub value: FileProcStatsValueType,
    #[tabled(rename = "Value")]
    pub human_value: String,
    #[tabled(skip)]
    pub raw_string_value: String,
}

impl FileProcStatsPrintItem {
    pub fn new(name: &str, human_name: Option<&str>, value: FileProcStatsValueType) -> Self {
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
            FileProcStatsValueType::Duration(duration) => { HumanDuration(*duration).to_string() }
            FileProcStatsValueType::Count(count) => { HumanCount(*count as u64).to_string() }
            FileProcStatsValueType::FileSize(size) => { HumanBytes(*size).to_string() }
            FileProcStatsValueType::SystemTime(time) => {
                let datetime = DateTime::<Utc>::from(*time);
                datetime.format("%Y-%m-%d %H:%M:%S").to_string()
            }
            FileProcStatsValueType::StringVec(vec) => { vec.join(", ") }
        };

        let raw_string_value = match &value {
            FileProcStatsValueType::Duration(duration) => {
                let total_seconds = duration.as_secs();
                let hours = total_seconds / 3600;
                let minutes = (total_seconds % 3600) / 60;
                let seconds = total_seconds % 60;
                let millis = duration.subsec_millis();
                format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
            }
            FileProcStatsValueType::Count(count) => { count.to_string() }
            FileProcStatsValueType::FileSize(size) => { size.to_string() }
            FileProcStatsValueType::SystemTime(time) => {
                let datetime = DateTime::<Utc>::from(*time);
                datetime.format("%Y-%m-%d %H:%M:%S").to_string()
            }
            FileProcStatsValueType::StringVec(vec) => { vec.join(", ") }
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
    fn get_elapsed(start: &Option<Instant>, end: &Option<Instant>) -> Duration {
        match (start, end) {
            (Some(start), Some(end)) => { end.duration_since(*start) }
            _ => Duration::new(0, 0),
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
    pub fn print(self) {
        let items = self.to_print_items();
        let table = Table::new(items).with(Style::psql()).to_string();
        println!("{}", table);
    }

    pub fn to_print_items(&self) -> Vec<FileProcStatsPrintItem> {
        let items = vec![
            FileProcStatsPrintItem::new(
                "run_start_time",
                None,
                FileProcStatsValueType::SystemTime(
                    FileProcStats::get_system_time(&self.run_start_time)
                )
            ),
            FileProcStatsPrintItem::new(
                "process_duration",
                None,
                FileProcStatsValueType::Duration(
                    FileProcStats::get_elapsed(&self.process_start, &self.process_finish)
                )
            ),
            FileProcStatsPrintItem::new(
                "scan_duration",
                None,
                FileProcStatsValueType::Duration(
                    FileProcStats::get_elapsed(&self.scan_start, &self.scan_finish)
                )
            ),
            FileProcStatsPrintItem::new(
                "scan_input_paths",
                None,
                FileProcStatsValueType::StringVec(self.scan_input_paths.clone())
            ),
            FileProcStatsPrintItem::new(
                "scan_file_count",
                None,
                FileProcStatsValueType::Count(self.scan_file_count)
            ),
            FileProcStatsPrintItem::new(
                "scan_file_size",
                None,
                FileProcStatsValueType::FileSize(self.scan_file_size)
            ),
            FileProcStatsPrintItem::new(
                "scan_size_dupe_file_count",
                None,
                FileProcStatsValueType::Count(self.scan_size_dupe_file_count)
            ),
            FileProcStatsPrintItem::new(
                "scan_size_dupe_file_size",
                None,
                FileProcStatsValueType::FileSize(self.scan_size_dupe_file_size)
            ),
            FileProcStatsPrintItem::new(
                "hash_duration",
                None,
                FileProcStatsValueType::Duration(
                    FileProcStats::get_elapsed(&self.hash_start, &self.hash_finish)
                )
            ),
            FileProcStatsPrintItem::new(
                "hash_scan_file_count",
                None,
                FileProcStatsValueType::Count(self.hash_scan_file_count)
            ),
            FileProcStatsPrintItem::new(
                "hash_scan_file_size",
                None,
                FileProcStatsValueType::FileSize(self.hash_scan_file_size)
            ),
            FileProcStatsPrintItem::new(
                "hash_cache_hit_full_count",
                None,
                FileProcStatsValueType::Count(self.hash_cache_hit_full_count)
            ),
            FileProcStatsPrintItem::new(
                "hash_cache_hit_partial_count",
                None,
                FileProcStatsValueType::Count(self.hash_cache_hit_partial_count)
            ),
            FileProcStatsPrintItem::new(
                "hash_gen_partial_count",
                None,
                FileProcStatsValueType::Count(self.hash_gen_partial_count)
            ),
            FileProcStatsPrintItem::new(
                "hash_gen_partial_duration",
                None,
                FileProcStatsValueType::Duration(self.hash_gen_partial_duration)
            ),
            FileProcStatsPrintItem::new(
                "hash_gen_partial_file_size",
                None,
                FileProcStatsValueType::FileSize(self.hash_gen_partial_file_size)
            ),
            FileProcStatsPrintItem::new(
                "hash_gen_full_count",
                None,
                FileProcStatsValueType::Count(self.hash_gen_full_count)
            ),
            FileProcStatsPrintItem::new(
                "hash_gen_full_file_size",
                None,
                FileProcStatsValueType::FileSize(self.hash_gen_full_file_size)
            ),
            FileProcStatsPrintItem::new(
                "hash_gen_full_duration",
                None,
                FileProcStatsValueType::Duration(self.hash_gen_full_duration)
            ),
            FileProcStatsPrintItem::new(
                "hash_confirmed_dupe_count",
                None,
                FileProcStatsValueType::Count(self.hash_confirmed_dupe_count)
            ),
            FileProcStatsPrintItem::new(
                "hash_confirmed_dupe_size",
                None,
                FileProcStatsValueType::FileSize(self.hash_confirmed_dupe_size)
            ),
            FileProcStatsPrintItem::new(
                "hash_confirmed_dupe_distinct_count",
                None,
                FileProcStatsValueType::Count(self.hash_confirmed_dupe_distinct_count)
            ),
            FileProcStatsPrintItem::new(
                "hash_confirmed_dupe_distinct_size",
                None,
                FileProcStatsValueType::FileSize(self.hash_confirmed_dupe_distinct_size)
            ),
            FileProcStatsPrintItem::new(
                "cache_map_to_dupe_vec_duration",
                None,
                FileProcStatsValueType::Duration(
                    FileProcStats::get_elapsed(
                        &self.cache_map_to_dupe_vec_start,
                        &self.cache_map_to_dupe_vec_finish
                    )
                )
            ),
            FileProcStatsPrintItem::new(
                "cache_map_to_dupe_vec_count",
                None,
                FileProcStatsValueType::Count(self.cache_map_to_dupe_vec_count)
            )
            // FileProcStatsPrintItem::new(
            //     "db_dupe_file_insert_duration",
            //     None,
            //     FileProcStatsValueType::Duration(
            //         FileProcStats::get_elapsed(
            //             &self.db_dupe_file_insert_start,
            //             &self.db_dupe_file_insert_finish
            //         )
            //     )
            // ),
            // FileProcStatsPrintItem::new(
            //     "db_dupe_file_insert_count",
            //     None,
            //     FileProcStatsValueType::Count(self.db_dupe_file_insert_count)
            // )
        ];

        items
    }

    pub fn write_csv(self, filename: &str) -> std::io::Result<()> {
        let file_exists = fs::metadata(filename).is_ok();

        let mut wtr = if file_exists {
            let file = OpenOptions::new().append(true).create(true).open(filename)?;
            csv::Writer::from_writer(file)
        } else {
            let file = fs::File::create(filename)?;
            csv::Writer::from_writer(file)
        };

        let items = self.to_print_items();

        if !file_exists {
            // Collect headers
            // let items = self.to_print_items();
            let mut headers = vec![];
            for item in &items {
                headers.push(&item.name);
            }

            // Write headers
            wtr.write_record(&headers)?;
        }

        // Collect values
        // let items = self.to_print_items();
        let mut values = vec![];
        for item in &items {
            values.push(&item.raw_string_value);
        }

        // Write values
        wtr.write_record(&values)?;

        wtr.flush()?;
        Ok(())
    }
}

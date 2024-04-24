use std::borrow::Cow;
use std::sync::Mutex;
use std::time::Duration;
use console::{ style, StyledObject };

use indicatif::MultiProgress;
use indicatif::{ HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle };

use serde::{ Deserialize, Serialize };
use std::fs::File;
use std::io::BufReader;

pub fn load_spinner_frames(name: String) -> &'static [&'static str] {
    // Open the JSON file
    let file = File::open("./spinners.json").ok().unwrap();
    let reader = BufReader::new(file);

    // Deserialize the JSON into a Frames struct
    let json_data: serde_json::Value = serde_json::from_reader(reader).ok().unwrap();

    // Extract the "dots" object
    let item_json = json_data.get(name).unwrap();

    // Deserialize the "dots" object into a Frames struct
    let frames: SpinnerConfig = serde_json::from_value(item_json.clone()).ok().unwrap();

    // Convert Vec<String> to Vec<&'static str>
    // Convert Vec<String> to Vec<&'static str>
    let frames_array: Vec<&'static str> = frames.frames
        .iter()
        .map(|s| s.as_str())
        .map(|s| Box::leak(s.to_owned().into_boxed_str()))
        .map(|s| s as &'static str) // Cloning mutable references into immutable ones
        .collect();

    let frames_slice: &'static [&'static str] = Box::leak(frames_array.into_boxed_slice());

    frames_slice
}

pub fn load_spinner(name: String) -> SpinnerConfig {
    // Open the JSON file
    let file = File::open("./spinners.json").ok().unwrap();
    let reader = BufReader::new(file);

    // Deserialize the JSON into a Frames struct
    let json_data: serde_json::Value = serde_json::from_reader(reader).ok().unwrap();

    // Extract the "dots" object
    let item_json = json_data.get(name).unwrap();

    // Deserialize the "dots" object into a Frames struct
    // let frames: SpinnerConfig =
    serde_json::from_value(item_json.clone()).ok().unwrap()
}

#[derive(Debug, Deserialize, Serialize)]
struct SpinnerConfig {
    interval: i32,
    frames: Vec<String>,
}

const STATUS_BAR_TYPE_COUNT: usize = 6;

// Define an enum to represent the task
#[derive(Debug, Copy, Clone)]
pub enum FileProcStatusType {
    Scan,
    Hash,
    HashBar,
    CacheToDupe,
    CacheToDupeBar,
    DbDupeFile,
}

impl FileProcStatusType {
    // Method to convert Task enum variant to array index
    fn to_index(self) -> usize {
        match self {
            FileProcStatusType::Scan => 0,
            FileProcStatusType::Hash => 1,
            FileProcStatusType::HashBar => 2,
            FileProcStatusType::CacheToDupe => 3,
            FileProcStatusType::CacheToDupeBar => 4,
            FileProcStatusType::DbDupeFile => 5,
        }
    }
}

// Implement indexing for Task enum
impl std::ops::Index<FileProcStatusType> for [ProgressBar; STATUS_BAR_TYPE_COUNT] {
    type Output = ProgressBar;

    fn index(&self, task: FileProcStatusType) -> &Self::Output {
        let index = task.to_index();
        &self[index]
    }
}

// Implement mutable indexing for Task enum
impl std::ops::IndexMut<FileProcStatusType> for [ProgressBar; STATUS_BAR_TYPE_COUNT] {
    fn index_mut(&mut self, task: FileProcStatusType) -> &mut Self::Output {
        let index = task.to_index();
        &mut self[index]
    }
}

pub struct FileProcStatusBars {}

const DEFAULT_SPINNER_TEMPLATE: &str =
    "[{elapsed_precise}] {spinner} {prefix:.bold.dim} {wide_msg}";

const DEFAULT_FINISH_TEMPLATE: &str = "[{elapsed_precise}] {msg}";
// const DEFAULT_STEADY_TICK_MS: u64 = 80;

static DEFAULT_STEADY_TICK_MS: Mutex<u64> = Mutex::new(80);

fn set_default_steady_tick(value: u64) {
    if let Ok(mut guard) = DEFAULT_STEADY_TICK_MS.lock() {
        *guard = value;
    }
}

// Function to get the value of the global variable
fn get_default_steady_tick() -> u64 {
    if let Ok(guard) = DEFAULT_STEADY_TICK_MS.lock() {
        *guard
    } else {
        0 // Return a default value if the lock fails
    }
}

impl FileProcStatusBars {
    // fn new_spinner(spinner_frames: &[&str]) -> ProgressBar {
    fn new_spinner(spinner_config: &SpinnerConfig) -> ProgressBar {
        let frames: Vec<&'static str> = spinner_config.frames
            .iter()
            .map(|s| s.as_str())
            .map(|s| Box::leak(s.to_owned().into_boxed_str()))
            .map(|s| s as &'static str) // Cloning mutable references into immutable ones
            .collect();

        let pb = ProgressBar::new_spinner();
        let spinner_style = ProgressStyle::with_template(DEFAULT_SPINNER_TEMPLATE)
            .unwrap()
            .tick_strings(&frames);

        pb.set_style(spinner_style);
        pb
    }

    fn new_progress_bar() -> ProgressBar {
        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                &format!(
                    "[{{elapsed_precise}}] {{prefix:.bold}}▕{{bar:.{}}}▏ {{wide_msg}}",
                    String::from("green")
                )
            )
                .unwrap()
                .progress_chars("█▓▒░  ")
        );
        pb
    }

    pub fn new_progress_bars() -> ([ProgressBar; STATUS_BAR_TYPE_COUNT], MultiProgress) {
        let m = MultiProgress::new();
        m.set_move_cursor(true);

        let spinner_frames_key = "dots";
        // let frames = load_spinner_frames(spinner_frames_key.to_string());
        let spinner_config = load_spinner(spinner_frames_key.to_string());
        // let frames = spinner_config.frames.as_slice();

        let bars: [ProgressBar; STATUS_BAR_TYPE_COUNT] = [
            m.add(FileProcStatusBars::new_spinner(&spinner_config)), // FileProcStatusType::Scan => 0,
            m.add(FileProcStatusBars::new_spinner(&spinner_config)), // FileProcStatusType::Hash => 1,
            m.add(FileProcStatusBars::new_progress_bar()), // FileProcStatusType::HashBar => 2,
            m.add(FileProcStatusBars::new_spinner(&spinner_config)), // FileProcStatusType::CacheToDupe => 3,
            m.add(FileProcStatusBars::new_progress_bar()), // FileProcStatusType::CacheToDupeBar => 4,
            m.add(FileProcStatusBars::new_progress_bar()), // FileProcStatusType::Db => 5,
        ];

        (bars, m)
    }

    pub fn new_finish_style() -> ProgressStyle {
        ProgressStyle::with_template(DEFAULT_FINISH_TEMPLATE).unwrap()
    }
}

pub fn to_count_style<T: std::fmt::Display>(count: T) -> StyledObject<HumanCount>
    where T: std::fmt::Display + Into<u64>
{
    style(HumanCount(count.into())).bold().green()
}

pub fn to_bytes_style<T: std::fmt::Display>(bytes: T) -> StyledObject<HumanBytes>
    where T: std::fmt::Display + Into<u64>
{
    style(HumanBytes(bytes.into())).bold().green()
}

pub fn to_duration_style(count: Duration) -> StyledObject<HumanDuration> {
    style(HumanDuration(count)).bold().red()
}

pub trait FileProcProgressBar {
    fn finish_with_finish_style(&self, message: impl Into<Cow<'static, str>>);
    fn enable_steady_tick_default(&self);
    fn to_count_style<T: std::fmt::Display>(&self, count: T) -> StyledObject<HumanCount>
        where T: std::fmt::Display + Into<u64>;
    fn to_bytes_style<T: std::fmt::Display>(&self, count: T) -> StyledObject<HumanBytes>
        where T: std::fmt::Display + Into<u64>;
    fn to_duration_style(&self, duration: Duration) -> StyledObject<HumanDuration>;
}

impl FileProcProgressBar for ProgressBar {
    fn finish_with_finish_style(&self, message: impl Into<Cow<'static, str>>) {
        self.set_style(FileProcStatusBars::new_finish_style());
        self.finish_with_message(message);
    }
    fn enable_steady_tick_default(&self) {
        self.enable_steady_tick(Duration::from_millis(get_default_steady_tick()));
    }
    fn to_count_style<T: std::fmt::Display>(&self, count: T) -> StyledObject<HumanCount>
        where T: std::fmt::Display + Into<u64>
    {
        style(HumanCount(count.into())).bold().green()
    }

    fn to_bytes_style<T: std::fmt::Display>(&self, bytes: T) -> StyledObject<HumanBytes>
        where T: std::fmt::Display + Into<u64>
    {
        style(HumanBytes(bytes.into())).bold().green()
    }

    fn to_duration_style(&self, count: Duration) -> StyledObject<HumanDuration> {
        style(HumanDuration(count)).bold().red()
        // style(HumanBytes(bytes.into())).bold().green()
    }
}

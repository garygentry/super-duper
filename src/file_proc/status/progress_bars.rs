use std::borrow::Cow;
use std::time::Duration;
use console::{ style, StyledObject };

use indicatif::MultiProgress;
use indicatif::{ HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle };

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
const DEFAULT_STEADY_TICK_MS: u64 = 100;

impl FileProcStatusBars {
    fn new_spinner() -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        let spinner_style = ProgressStyle::with_template(DEFAULT_SPINNER_TEMPLATE)
            .unwrap()
            // .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
            .tick_strings(&[".  ", ".. ", "...", " ..", "  .", "   "]);

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

        let bars: [ProgressBar; STATUS_BAR_TYPE_COUNT] = [
            m.add(FileProcStatusBars::new_spinner()), // FileProcStatusType::Scan => 0,
            m.add(FileProcStatusBars::new_spinner()), // FileProcStatusType::Hash => 1,
            m.add(FileProcStatusBars::new_progress_bar()), // FileProcStatusType::HashBar => 2,
            m.add(FileProcStatusBars::new_spinner()), // FileProcStatusType::CacheToDupe => 3,
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
        self.enable_steady_tick(Duration::from_millis(DEFAULT_STEADY_TICK_MS));
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

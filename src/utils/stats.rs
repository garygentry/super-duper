use std::time::{ Duration, Instant };
use indicatif::HumanDuration;

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
        self.duration = self.finish_time
            .unwrap()
            .duration_since(self.start_time.unwrap());
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

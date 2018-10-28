use std::io::{stdout, Write};
use std::time::Instant;
use PROGRESS_FREQUENCY_SECONDS;

pub fn elapsed_seconds(since: Instant) -> f64 {
    let dt = since.elapsed();
    (dt.as_secs() as f64) + (f64::from(dt.subsec_nanos()) * 1e-9)
}

pub struct Progress {
    label: String,
    processed_items: usize,
    total_items: usize,
    started_at: Instant,
    last_printed_at: Instant,
}

impl Progress {
    pub fn new(label: &str, total_items: usize) -> Progress {
        Progress {
            label: label.to_string(),
            processed_items: 0,
            total_items,
            started_at: Instant::now(),
            last_printed_at: Instant::now(),
        }
    }

    pub fn next(&mut self) {
        self.processed_items += 1;
        if self.processed_items > self.total_items {
            panic!(
                "{} is too few items for {} progress",
                self.total_items, self.label
            );
        }

        let done = self.processed_items == self.total_items;
        if elapsed_seconds(self.last_printed_at) >= PROGRESS_FREQUENCY_SECONDS || done {
            self.last_printed_at = Instant::now();
            // TODO blank till end of current line
            print!(
                "{}{}: {}/{}... {}s",
                "\r",
                self.label,
                self.processed_items,
                self.total_items,
                elapsed_seconds(self.started_at),
            );
            if done {
                println!("");
            } else {
                stdout().flush().unwrap();
            }
        }
    }
}

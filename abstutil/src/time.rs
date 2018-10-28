use std::io::{stdout, Write};
use std::time::Instant;
use PROGRESS_FREQUENCY_SECONDS;

pub fn elapsed_seconds(since: Instant) -> f64 {
    let dt = since.elapsed();
    (dt.as_secs() as f64) + (f64::from(dt.subsec_nanos()) * 1e-9)
}

struct Progress {
    label: String,
    processed_items: usize,
    total_items: usize,
    started_at: Instant,
    last_printed_at: Instant,
}

impl Progress {
    fn new(label: &str, total_items: usize) -> Progress {
        Progress {
            label: label.to_string(),
            processed_items: 0,
            total_items,
            started_at: Instant::now(),
            last_printed_at: Instant::now(),
        }
    }

    // Returns when done
    fn next(&mut self, padding: String) -> Option<String> {
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
            let line = format!(
                "{}{}: {}/{}... {}s",
                padding,
                self.label,
                self.processed_items,
                self.total_items,
                elapsed_seconds(self.started_at)
            );
            // TODO blank till end of current line
            print!("\r{}", line);
            if done {
                println!("");
                return Some(line);
            } else {
                stdout().flush().unwrap();
            }
        }
        None
    }
}

enum StackEntry {
    TimerSpan(TimerSpan),
    Progress(Progress),
}

// Hierarchial magic
pub struct Timer {
    results: Vec<String>,
    stack: Vec<StackEntry>,
}

struct TimerSpan {
    name: String,
    started_at: Instant,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            results: Vec::new(),
            stack: Vec::new(),
        }
    }

    pub fn done(self) {
        assert!(self.stack.is_empty());
        println!("");
        for line in self.results {
            println!("{}", line);
        }
        println!("");
    }

    pub fn start(&mut self, name: &str) {
        println!("{}- {}...", "  ".repeat(self.stack.len()), name);
        self.stack.push(StackEntry::TimerSpan(TimerSpan {
            name: name.to_string(),
            started_at: Instant::now(),
        }));
    }

    pub fn stop(&mut self, name: &str) {
        let span = match self.stack.pop().unwrap() {
            StackEntry::TimerSpan(s) => s,
            StackEntry::Progress(p) => panic!(
                "stop({}) while a Progress({}, {}/{}) is top of the stack",
                name, p.label, p.processed_items, p.total_items
            ),
        };
        assert_eq!(span.name, name);
        let line = format!(
            "{}- {} took {}s",
            "  ".repeat(self.stack.len()),
            name,
            elapsed_seconds(span.started_at)
        );
        println!("{}", line);
        self.results.push(line);
    }

    pub fn start_iter(&mut self, name: &str, total_items: usize) {
        if total_items == 0 {
            panic!("Can't start_iter({}, 0)", name);
        }
        if let Some(StackEntry::Progress(p)) = self.stack.last() {
            panic!(
                "Can't start_iter({}) while Progress({}) is top of the stack",
                name, p.label
            );
        }

        self.stack
            .push(StackEntry::Progress(Progress::new(name, total_items)));
    }

    pub fn next(&mut self) {
        let padding = format!("{} - ", "  ".repeat(self.stack.len() - 1));
        let done = if let Some(StackEntry::Progress(ref mut progress)) = self.stack.last_mut() {
            if let Some(result) = progress.next(padding) {
                self.results.push(result);
                true
            } else {
                false
            }
        } else {
            panic!("Can't next() while a TimerSpan is top of the stack");
        };
        if done {
            self.stack.pop();
        }
    }
}

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
    fn next(&mut self) -> Option<(f64, String)> {
        self.processed_items += 1;
        if self.processed_items > self.total_items {
            panic!(
                "{} is too few items for {} progress",
                self.total_items, self.label
            );
        }

        if self.processed_items == self.total_items {
            let elapsed = elapsed_seconds(self.started_at);
            let line = format!("{} ({})... {}s", self.label, self.total_items, elapsed);
            // TODO blank till end of current line
            println!("\r{}", line);
            return Some((elapsed, line));
        } else if elapsed_seconds(self.last_printed_at) >= PROGRESS_FREQUENCY_SECONDS {
            self.last_printed_at = Instant::now();
            // TODO blank till end of current line
            print!(
                "\r{}: {}/{}... {:.1}s",
                self.label,
                self.processed_items,
                self.total_items,
                elapsed_seconds(self.started_at)
            );
            stdout().flush().unwrap();
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

    outermost_name: String,
}

struct TimerSpan {
    name: String,
    started_at: Instant,
    nested_results: Vec<String>,
    nested_time: f64,
}

impl Timer {
    pub fn new(name: String) -> Timer {
        let mut t = Timer {
            results: Vec::new(),
            stack: Vec::new(),
            outermost_name: name.clone(),
        };
        t.start(&name);
        t
    }

    pub fn done(mut self) {
        let stop_name = self.outermost_name.clone();
        self.stop(&stop_name);
        assert!(self.stack.is_empty());
        println!("");
        for line in self.results {
            println!("{}", line);
        }
        println!("");
    }

    pub fn start(&mut self, name: &str) {
        println!("{}...", name);
        self.stack.push(StackEntry::TimerSpan(TimerSpan {
            name: name.to_string(),
            started_at: Instant::now(),
            nested_results: Vec::new(),
            nested_time: 0.0,
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
        let elapsed = elapsed_seconds(span.started_at);
        let line = format!("{} took {}s", name, elapsed);

        let padding = "  ".repeat(self.stack.len());
        match self.stack.last_mut() {
            Some(StackEntry::TimerSpan(ref mut s)) => {
                s.nested_results.push(format!("{}- {}", padding, line));
                s.nested_results.extend(span.nested_results);
                if span.nested_time != 0.0 {
                    println!("{}... plus {}s", name, elapsed - span.nested_time);
                    s.nested_results.push(format!(
                        "  {}- ... plus {}s",
                        padding,
                        elapsed - span.nested_time
                    ));
                }
                s.nested_time += elapsed;
            }
            Some(StackEntry::Progress(p)) => panic!(
                "How is TimerSpan({}) nested under Progress({})?",
                name, p.label
            ),
            None => {
                self.results.push(format!("{}- {}", padding, line));
                self.results.extend(span.nested_results);
                if span.nested_time != 0.0 {
                    println!("{}... plus {}s", name, elapsed - span.nested_time);
                    self.results
                        .push(format!("  - ... plus {}s", elapsed - span.nested_time));
                }
                // Don't bother tracking excess time that the Timer has existed but had no spans
            }
        }

        println!("{}", line);
    }

    pub fn start_iter(&mut self, name: &str, total_items: usize) {
        if total_items == 0 {
            return;
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
        let maybe_result =
            if let Some(StackEntry::Progress(ref mut progress)) = self.stack.last_mut() {
                progress.next()
            } else {
                panic!("Can't next() while a TimerSpan is top of the stack");
            };
        if let Some((elapsed, result)) = maybe_result {
            self.stack.pop();
            self.add_result(elapsed, result);
        }
    }

    pub(crate) fn add_result(&mut self, elapsed: f64, line: String) {
        let padding = "  ".repeat(self.stack.len());
        match self.stack.last_mut() {
            Some(StackEntry::TimerSpan(ref mut s)) => {
                s.nested_results.push(format!("{}- {}", padding, line));
                s.nested_time += elapsed;
            }
            Some(StackEntry::Progress(p)) => {
                panic!("How is something nested under Progress({})?", p.label)
            }
            None => {
                self.results.push(format!("{}- {}", padding, line));
                // Don't bother tracking excess time that the Timer has existed but had no spans
            }
        }
    }
}

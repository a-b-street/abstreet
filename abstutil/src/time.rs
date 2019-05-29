use crate::{notes, PROGRESS_FREQUENCY_SECONDS};
use std::collections::HashMap;
use std::fs::File;
use std::io::{stdout, BufReader, Error, Read, Write};
use std::time::Instant;

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
    first_update: bool,
}

impl Progress {
    fn new(label: &str, total_items: usize) -> Progress {
        Progress {
            label: label.to_string(),
            processed_items: 0,
            total_items,
            started_at: Instant::now(),
            last_printed_at: Instant::now(),
            first_update: true,
        }
    }

    // Returns when done
    fn next<'a>(&mut self, maybe_sink: &mut Option<Box<TimerSink + 'a>>) -> Option<(f64, String)> {
        self.processed_items += 1;
        if self.processed_items > self.total_items {
            panic!(
                "{} is too few items for {} progress",
                prettyprint_usize(self.total_items),
                self.label
            );
        }

        if self.processed_items == self.total_items {
            let elapsed = elapsed_seconds(self.started_at);
            let line = format!(
                "{} ({})... {}",
                self.label,
                prettyprint_usize(self.total_items),
                prettyprint_time(elapsed)
            );
            if self.total_items == 1 {
                Timer::selfless_println(maybe_sink, line.clone());
            } else {
                clear_current_line();
                println!("{}", line);
                if let Some(ref mut sink) = maybe_sink {
                    sink.reprintln(line.clone());
                }
            }
            return Some((elapsed, line));
        } else if elapsed_seconds(self.last_printed_at) >= PROGRESS_FREQUENCY_SECONDS {
            self.last_printed_at = Instant::now();
            let line = format!(
                "{}: {}/{}... {}",
                self.label,
                prettyprint_usize(self.processed_items),
                prettyprint_usize(self.total_items),
                prettyprint_time(elapsed_seconds(self.started_at))
            );
            clear_current_line();
            print!("{}", line);
            stdout().flush().unwrap();

            if let Some(ref mut sink) = maybe_sink {
                if self.first_update {
                    sink.println(line);
                    self.first_update = false;
                } else {
                    sink.reprintln(line);
                }
            }
        }
        None
    }
}

enum StackEntry {
    TimerSpan(TimerSpan),
    Progress(Progress),
    File(TimedFileReader),
}

pub trait TimerSink {
    fn println(&mut self, line: String);
    fn reprintln(&mut self, line: String);
}

// Hierarchial magic
pub struct Timer<'a> {
    results: Vec<String>,
    stack: Vec<StackEntry>,

    outermost_name: String,

    notes: Vec<String>,
    pub(crate) warnings: Vec<String>,

    sink: Option<Box<TimerSink + 'a>>,
}

struct TimerSpan {
    name: String,
    started_at: Instant,
    nested_results: Vec<String>,
    nested_time: f64,
}

impl<'a> Timer<'a> {
    pub fn new(name: &str) -> Timer<'a> {
        let mut t = Timer {
            results: Vec::new(),
            stack: Vec::new(),
            outermost_name: name.to_string(),
            notes: Vec::new(),
            warnings: Vec::new(),
            sink: None,
        };
        t.start(name);
        t
    }

    pub fn new_with_sink(name: &str, sink: Box<TimerSink + 'a>) -> Timer<'a> {
        let mut t = Timer::new(name);
        t.sink = Some(sink);
        t
    }

    // TODO Shouldn't use this much.
    pub fn throwaway() -> Timer<'a> {
        Timer::new("throwaway")
    }

    fn println(&mut self, line: String) {
        Timer::selfless_println(&mut self.sink, line);
    }

    // Workaround for borrow checker
    fn selfless_println(maybe_sink: &mut Option<Box<TimerSink + 'a>>, line: String) {
        println!("{}", line);
        if let Some(ref mut sink) = maybe_sink {
            sink.println(line);
        }
    }

    // Log immediately, but also repeat at the end, to avoid having to scroll up and find
    // interesting debug stuff.
    pub fn note(&mut self, line: String) {
        // Interrupt the start_iter with a newline.
        if let Some(StackEntry::Progress(_)) = self.stack.last() {
            self.println(String::new());
        }

        self.println(line.clone());
        self.notes.push(line);
    }

    pub fn warn(&mut self, line: String) {
        self.warnings.push(line);
    }

    // Used to end the scope of a timer early.
    pub fn done(self) {}

    pub fn start(&mut self, name: &str) {
        self.println(format!("{}...", name));
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
            _ => unreachable!(),
        };
        assert_eq!(span.name, name);
        let elapsed = elapsed_seconds(span.started_at);
        let line = format!("{} took {}", name, prettyprint_time(elapsed));

        let padding = "  ".repeat(self.stack.len());
        match self.stack.last_mut() {
            Some(StackEntry::TimerSpan(ref mut s)) => {
                s.nested_results.push(format!("{}- {}", padding, line));
                s.nested_results.extend(span.nested_results);
                if span.nested_time != 0.0 {
                    Timer::selfless_println(
                        &mut self.sink,
                        format!(
                            "{}... plus {}",
                            name,
                            prettyprint_time(elapsed - span.nested_time)
                        ),
                    );
                    s.nested_results.push(format!(
                        "  {}- ... plus {}",
                        padding,
                        prettyprint_time(elapsed - span.nested_time)
                    ));
                }
                s.nested_time += elapsed;
            }
            Some(_) => unreachable!(),
            None => {
                self.results.push(format!("{}- {}", padding, line));
                self.results.extend(span.nested_results);
                if span.nested_time != 0.0 {
                    self.println(format!(
                        "{}... plus {}",
                        name,
                        prettyprint_time(elapsed - span.nested_time)
                    ));
                    self.results.push(format!(
                        "  - ... plus {}",
                        prettyprint_time(elapsed - span.nested_time)
                    ));
                }
                // Don't bother tracking excess time that the Timer has existed but had no spans
            }
        }

        self.println(line);
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
                progress.next(&mut self.sink)
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
            Some(_) => unreachable!(),
            None => {
                self.results.push(format!("{}- {}", padding, line));
                // Don't bother tracking excess time that the Timer has existed but had no spans
            }
        }
    }

    pub fn parallelize<I, O, F: Fn(I) -> O>(
        &mut self,
        timer_name: &str,
        requests: Vec<I>,
        cb: F,
    ) -> Vec<O>
    where
        I: Send,
        O: Send,
        F: Send + Clone + Copy,
    {
        scoped_threadpool::Pool::new(num_cpus::get() as u32).scoped(|scope| {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut results: Vec<Option<O>> = std::iter::repeat_with(|| None)
                .take(requests.len())
                .collect();
            for (idx, req) in requests.into_iter().enumerate() {
                let tx = tx.clone();
                scope.execute(move || {
                    tx.send((idx, cb(req))).unwrap();
                });
            }
            drop(tx);

            self.start_iter(timer_name, results.len());
            for (idx, result) in rx.iter() {
                self.next();
                results[idx] = Some(result);
            }
            results.into_iter().map(|x| x.unwrap()).collect()
        })
    }

    // Then the caller passes this in as a reader
    pub fn read_file(&mut self, path: &str) -> Result<(), Error> {
        self.stack
            .push(StackEntry::File(TimedFileReader::new(path)?));
        Ok(())
    }
}

impl<'a> std::ops::Drop for Timer<'a> {
    fn drop(&mut self) {
        let stop_name = self.outermost_name.clone();

        // If we're in the middle of unwinding a panic, don't further blow up.
        match self.stack.last() {
            Some(StackEntry::TimerSpan(ref s)) => {
                if s.name != stop_name {
                    println!("dropping Timer because of panic");
                    return;
                }
            }
            Some(_) => {
                println!("dropping Timer because of panic");
                return;
            }
            None => unreachable!(),
        }

        self.stop(&stop_name);
        assert!(self.stack.is_empty());
        self.println(String::new());
        for line in &self.results {
            Timer::selfless_println(&mut self.sink, line.to_string());
        }
        self.println(String::new());

        if !self.notes.is_empty() {
            self.println(format!("{} notes:", self.notes.len()));
            for line in &self.notes {
                Timer::selfless_println(&mut self.sink, line.to_string());
            }
            self.println(String::new());
        }
        notes::dump_notes();

        if !self.warnings.is_empty() {
            self.println(format!("{} warnings:", self.warnings.len()));
            for line in &self.warnings {
                Timer::selfless_println(&mut self.sink, line.to_string());
            }
            self.println(String::new());
        }
    }
}

// For repeated things
// TODO Why does the PartialEq derivation in sim require this?
pub struct Profiler {
    entries: Vec<ProfilerEntry>,
    current_entries: HashMap<String, Instant>,
}

struct ProfilerEntry {
    name: String,
    total_seconds: f64,
    rounds: usize,
}

impl Profiler {
    pub fn new() -> Profiler {
        Profiler {
            entries: Vec::new(),
            current_entries: HashMap::new(),
        }
    }

    // TODO Nested stuff winds up sorted before the parent
    pub fn start(&mut self, name: &str) {
        if self.current_entries.contains_key(name) {
            panic!(
                "Can't start profiling {}; it's already being recorded",
                name
            );
        }
        self.current_entries
            .insert(name.to_string(), Instant::now());
    }

    pub fn stop(&mut self, name: &str) {
        let start = self.current_entries.remove(name).expect(&format!(
            "Can't stop profiling {}, because it was never started",
            name
        ));
        let duration = elapsed_seconds(start);

        if let Some(ref mut entry) = self.entries.iter_mut().find(|e| e.name == name) {
            entry.total_seconds += duration;
            entry.rounds += 1;
        } else {
            self.entries.push(ProfilerEntry {
                name: name.to_string(),
                total_seconds: duration,
                rounds: 1,
            });
        }
    }

    pub fn dump(&self) {
        if !self.current_entries.is_empty() {
            panic!(
                "Can't dump Profiler with active entries {:?}",
                self.current_entries.keys()
            );
        }

        println!("Profiler results so far:");
        for entry in &self.entries {
            // Suppress things that don't take any time.
            let time_per_round = entry.total_seconds / (entry.rounds as f64);
            if time_per_round < 0.0001 {
                // TODO Actually, the granularity of the rounds might differ. Don't do this.
                //continue;
            }

            println!(
                "  - {}: {} over {} rounds ({} / round)",
                entry.name,
                prettyprint_time(entry.total_seconds),
                prettyprint_usize(entry.rounds),
                prettyprint_time(time_per_round),
            );
        }
    }
}

pub fn prettyprint_usize(x: usize) -> String {
    let num = format!("{}", x);
    let mut result = String::new();
    let mut i = num.len();
    for c in num.chars() {
        result.push(c);
        i -= 1;
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
    }
    result
}

pub fn prettyprint_time(seconds: f64) -> String {
    format!("{:.4}s", seconds)
}

// TODO This is an awful way to measure memory usage, but I can't find anything else that works.
pub struct MeasureMemory {
    before_mb: usize,
}

impl MeasureMemory {
    pub fn new() -> MeasureMemory {
        MeasureMemory {
            before_mb: process_used_memory_mb(),
        }
    }

    pub fn reset(&mut self, section: &str, timer: &mut Timer) {
        let now_mb = process_used_memory_mb();
        if now_mb >= self.before_mb {
            timer.note(format!(
                "{} cost ~{} MB",
                section,
                prettyprint_usize(now_mb - self.before_mb)
            ));
        } else {
            timer.note(format!(
                "WEIRD! {} freed up ~{} MB",
                section,
                prettyprint_usize(self.before_mb - now_mb)
            ));
        }
        self.before_mb = now_mb;
    }
}

#[cfg(target_os = "linux")]
fn process_used_memory_mb() -> usize {
    (procfs::Process::myself().unwrap().stat.vsize / 1024 / 1024) as usize
}

#[cfg(not(target_os = "linux"))]
fn process_used_memory_mb() -> usize {
    0
}

#[cfg(unix)]
pub(crate) fn clear_current_line() {
    // Fails in the test runner.
    if let Ok((terminal_width, _)) = termion::terminal_size() {
        print!(
            "{}{}",
            termion::clear::CurrentLine,
            termion::cursor::Left(terminal_width)
        );
    } else {
        print!("\r");
    }
}

#[cfg(windows)]
pub(crate) fn clear_current_line() {
    print!("\r");
}

struct TimedFileReader {
    inner: BufReader<File>,

    path: String,
    processed_bytes: usize,
    total_bytes: usize,
    started_at: Instant,
    last_printed_at: Option<Instant>,
}

impl TimedFileReader {
    fn new(path: &str) -> Result<TimedFileReader, Error> {
        let file = File::open(path)?;
        let total_bytes = file.metadata()?.len() as usize;
        Ok(TimedFileReader {
            inner: BufReader::new(file),
            path: path.to_string(),
            processed_bytes: 0,
            total_bytes,
            started_at: Instant::now(),
            last_printed_at: None,
        })
    }
}

impl<'a> Read for Timer<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut file = match self.stack.last_mut() {
            Some(StackEntry::File(ref mut f)) => f,
            _ => unreachable!(),
        };

        let bytes = file.inner.read(buf)?;
        file.processed_bytes += bytes;
        if file.processed_bytes > file.total_bytes {
            panic!(
                "{} is too many bytes read from {}",
                prettyprint_usize(file.processed_bytes),
                file.path
            );
        }

        if file.processed_bytes == file.total_bytes {
            let elapsed = elapsed_seconds(file.started_at);
            let line = format!(
                "Read {} ({})... {}",
                file.path,
                prettyprint_usize(file.total_bytes / 1024 / 1024),
                prettyprint_time(elapsed)
            );
            if file.last_printed_at.is_none() {
                self.println(line.clone());
            } else {
                clear_current_line();
                println!("{}", line);
                if let Some(ref mut sink) = self.sink {
                    sink.reprintln(line.clone());
                }
            }
            self.stack.pop();
            self.add_result(elapsed, line);
        } else if file.last_printed_at.is_none()
            || elapsed_seconds(file.last_printed_at.unwrap()) >= PROGRESS_FREQUENCY_SECONDS
        {
            let line = format!(
                "Reading {}: {}/{} MB... {}",
                file.path,
                prettyprint_usize(file.processed_bytes / 1024 / 1024),
                prettyprint_usize(file.total_bytes / 1024 / 1024),
                prettyprint_time(elapsed_seconds(file.started_at))
            );
            // TODO Refactor this pattern...
            clear_current_line();
            print!("{}", line);
            stdout().flush().unwrap();

            if let Some(ref mut sink) = self.sink {
                if file.last_printed_at.is_none() {
                    sink.println(line);
                } else {
                    sink.reprintln(line);
                }
            }

            file.last_printed_at = Some(Instant::now());
        }

        Ok(bytes)
    }
}

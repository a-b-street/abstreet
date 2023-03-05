use std::collections::BTreeMap;
use std::io::{stdout, BufReader, ErrorKind, Read, Write};

use anyhow::{Context, Result};
use fs_err::File;
use instant::Instant;

use crate::{prettyprint_usize, PROGRESS_FREQUENCY_SECONDS};

pub fn elapsed_seconds(since: Instant) -> f64 {
    let dt = since.elapsed();
    (dt.as_secs() as f64) + (f64::from(dt.subsec_nanos()) * 1e-9)
}

#[derive(Debug)]
struct Progress {
    label: String,
    processed_items: usize,
    total_items: usize,
    started_at: Instant,
    last_printed_at: Instant,
    first_update: bool,
}

impl Progress {
    fn new(label: String, total_items: usize) -> Progress {
        Progress {
            label,
            processed_items: 0,
            total_items,
            started_at: Instant::now(),
            last_printed_at: Instant::now(),
            first_update: true,
        }
    }

    // Returns when done
    fn next<'a>(
        &mut self,
        maybe_sink: &mut Option<Box<dyn TimerSink + 'a>>,
    ) -> Option<(f64, String)> {
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
                temporary_println(maybe_sink, line.clone());
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

    fn cancel_iter_early(&mut self) -> f64 {
        elapsed_seconds(self.started_at)
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

/// Hierarchial magic
pub struct Timer<'a> {
    results: Vec<String>,
    stack: Vec<StackEntry>,

    outermost_name: String,

    sink: Option<Box<dyn TimerSink + 'a>>,
}

struct TimerSpan {
    name: String,
    started_at: Instant,
    nested_results: Vec<String>,
    nested_time: f64,
}

impl<'a> Timer<'a> {
    pub fn new<S: Into<String>>(raw_name: S) -> Timer<'a> {
        let name = raw_name.into();
        let mut t = Timer {
            results: Vec::new(),
            stack: Vec::new(),
            outermost_name: name.clone(),
            sink: None,
        };
        t.start(name);
        t
    }

    pub fn new_with_sink(name: &str, sink: Box<dyn TimerSink + 'a>) -> Timer<'a> {
        let mut t = Timer::new(name);
        t.sink = Some(sink);
        t
    }

    // TODO Shouldn't use this much.
    pub fn throwaway() -> Timer<'a> {
        Timer::new("throwaway")
    }

    fn temporary_println(&mut self, line: String) {
        temporary_println(&mut self.sink, line);
    }

    /// Used to end the scope of a timer early.
    pub fn done(self) {}

    pub fn start<S: Into<String>>(&mut self, raw_name: S) {
        if self.outermost_name == "throwaway" {
            return;
        }

        let name = raw_name.into();
        self.temporary_println(format!("{}...", name));
        self.stack.push(StackEntry::TimerSpan(TimerSpan {
            name,
            started_at: Instant::now(),
            nested_results: Vec::new(),
            nested_time: 0.0,
        }));
    }

    pub fn stop<S: Into<String>>(&mut self, raw_name: S) {
        if self.outermost_name == "throwaway" {
            return;
        }
        let name = raw_name.into();

        let span = match self.stack.pop().unwrap() {
            StackEntry::TimerSpan(s) => s,
            StackEntry::Progress(p) => panic!("stop() during unfinished start_iter(): {:?}", p),
            StackEntry::File(f) => panic!("stop() while reading file {}", f.path),
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
                    temporary_println(
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
                    self.temporary_println(format!(
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

        self.temporary_println(line);
    }

    pub fn start_iter<S: Into<String>>(&mut self, raw_name: S, total_items: usize) {
        if self.outermost_name == "throwaway" {
            return;
        }
        if total_items == 0 {
            return;
        }
        let name = raw_name.into();
        // Note we may have two StackEntry::Progress entries nested

        self.stack
            .push(StackEntry::Progress(Progress::new(name, total_items)));
    }

    pub fn next(&mut self) {
        if self.outermost_name == "throwaway" {
            return;
        }
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

    pub fn cancel_iter_early(&mut self) {
        if self.outermost_name == "throwaway" {
            return;
        }
        let elapsed = if let Some(StackEntry::Progress(ref mut progress)) = self.stack.last_mut() {
            progress.cancel_iter_early()
        } else {
            panic!("Can't cancel_iter_early() while a TimerSpan is top of the stack");
        };
        self.stack.pop();
        self.add_result(elapsed, "cancelled early".to_string());
    }

    pub fn add_result(&mut self, elapsed: f64, line: String) {
        let padding = "  ".repeat(self.stack.len());
        match self.stack.last_mut() {
            Some(StackEntry::TimerSpan(ref mut s)) => {
                s.nested_results.push(format!("{}- {}", padding, line));
                s.nested_time += elapsed;
            }
            Some(StackEntry::Progress(_)) => {}
            Some(_) => unreachable!(),
            None => {
                self.results.push(format!("{}- {}", padding, line));
                // Don't bother tracking excess time that the Timer has existed but had no spans
            }
        }
    }

    /// Execute the callback over all requests, using all CPUs available. The order of the result
    /// is deterministic and matches the input.
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
        self.inner_parallelize(timer_name, requests, cb, num_cpus::get().max(1) as u32)
    }

    /// Like `parallelize`, but leave one CPU free, to avoid thrashing the user's system.
    pub fn parallelize_polite<I, O, F: Fn(I) -> O>(
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
        self.inner_parallelize(
            timer_name,
            requests,
            cb,
            (num_cpus::get() - 1).max(1) as u32,
        )
    }

    fn inner_parallelize<I, O, F: Fn(I) -> O>(
        &mut self,
        timer_name: &str,
        requests: Vec<I>,
        cb: F,
        num_cpus: u32,
    ) -> Vec<O>
    where
        I: Send,
        O: Send,
        F: Send + Clone + Copy,
    {
        // Here's the sequential equivalent, to conveniently compare times. Also gotta use this in
        // wasm; no threads.
        #[cfg(target_arch = "wasm32")]
        {
            // Silence a warning
            let _ = num_cpus;

            let mut results: Vec<O> = Vec::new();
            self.start_iter(timer_name, requests.len());
            for req in requests {
                self.next();
                results.push(cb(req));
            }
            return results;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            scoped_threadpool::Pool::new(num_cpus).scoped(|scope| {
                let (tx, rx) = std::sync::mpsc::channel();
                let mut results: Vec<Option<O>> = std::iter::repeat_with(|| None)
                    .take(requests.len())
                    .collect();
                for (idx, req) in requests.into_iter().enumerate() {
                    let tx = tx.clone();
                    scope.execute(move || {
                        // TODO Can we catch panics here, dump a better stacktrace? widgetry runner
                        // does this
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
    }

    /// Like BTreeMap::retain, but parallelized
    pub fn retain_parallelized<K, V, F: Fn(&V) -> bool>(
        &mut self,
        timer_name: &str,
        input: BTreeMap<K, V>,
        keep: F,
    ) -> BTreeMap<K, V>
    where
        K: Send + Ord,
        V: Send,
        F: Send + Sync + Clone + Copy,
    {
        self.parallelize(timer_name, input.into_iter().collect(), |(k, v)| {
            if keep(&v) {
                Some((k, v))
            } else {
                None
            }
        })
        .into_iter()
        .flatten()
        .collect()
    }

    /// Then the caller passes this in as a reader
    pub fn read_file(&mut self, path: &str) -> Result<()> {
        self.stack
            .push(StackEntry::File(TimedFileReader::new(path)?));
        Ok(())
    }
}

impl<'a> std::ops::Drop for Timer<'a> {
    fn drop(&mut self) {
        if self.outermost_name == "throwaway" {
            return;
        }

        let stop_name = self.outermost_name.clone();

        // If we're in the middle of unwinding a panic, don't further blow up.
        match self.stack.last() {
            Some(StackEntry::TimerSpan(ref s)) => {
                if s.name != stop_name {
                    error!("dropping Timer during {}, due to panic?", s.name);
                    return;
                }
            }
            Some(StackEntry::File(ref r)) => {
                error!("dropping Timer while reading {}, due to panic?", r.path);
                return;
            }
            Some(StackEntry::Progress(ref p)) => {
                error!(
                    "dropping Timer while doing progress {}, due to panic?",
                    p.label
                );
                return;
            }
            None => unreachable!(),
        }

        self.stop(&stop_name);
        assert!(self.stack.is_empty());
        for line in &self.results {
            finalized_println(&mut self.sink, line.to_string());
        }

        if std::thread::panicking() {
            error!("");
            error!("");
            error!("");
            error!("");
            error!("");
            error!("!!! The program panicked, look above for the stack trace !!!");
        }
    }
}

pub fn prettyprint_time(seconds: f64) -> String {
    format!("{:.4}s", seconds)
}

#[cfg(unix)]
pub fn clear_current_line() {
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

#[cfg(not(unix))]
pub fn clear_current_line() {
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
    fn new(path: &str) -> Result<TimedFileReader> {
        || -> Result<TimedFileReader> {
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
        }()
        .with_context(|| path.to_string())
    }
}

impl<'a> Read for Timer<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut file = match self.stack.last_mut() {
            Some(StackEntry::File(ref mut f)) => f,
            _ => {
                return Err(std::io::Error::new(
                    ErrorKind::Other,
                    "trying to read when Timer doesn't have file on the stack?!",
                ));
            }
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
            if self.outermost_name != "throwaway" {
                if file.last_printed_at.is_none() {
                    self.temporary_println(line.clone());
                } else {
                    clear_current_line();
                    println!("{}", line);
                    if let Some(ref mut sink) = self.sink {
                        sink.reprintln(line.clone());
                    }
                }
            }
            self.stack.pop();
            self.add_result(elapsed, line);
        } else if file.last_printed_at.is_none()
            || elapsed_seconds(file.last_printed_at.unwrap()) >= PROGRESS_FREQUENCY_SECONDS
        {
            if self.outermost_name != "throwaway" {
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
            }

            file.last_printed_at = Some(Instant::now());
        }

        Ok(bytes)
    }
}

// Print progress info while a Timer is still active. Invisible on web by default.
fn temporary_println<'a>(maybe_sink: &mut Option<Box<dyn TimerSink + 'a>>, line: String) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("{}", line);
    }
    #[cfg(target_arch = "wasm32")]
    {
        debug!("{}", line);
    }
    if let Some(ref mut sink) = maybe_sink {
        sink.println(line);
    }
}

// Print info about a completed Timer. Always uses info logs, so works on native and web.
fn finalized_println<'a>(maybe_sink: &mut Option<Box<dyn TimerSink + 'a>>, line: String) {
    info!("{}", line);
    if let Some(ref mut sink) = maybe_sink {
        sink.println(line);
    }
}

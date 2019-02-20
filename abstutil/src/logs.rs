use crate::Timer;
use log::{Level, Log, Metadata, Record};
use yansi::Paint;

pub struct LogAdapter;

impl Log for LogAdapter {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        println!("{}", format_log_record(record));
    }

    fn flush(&self) {}
}

pub fn format_log_record(record: &Record) -> String {
    format!(
        "[{}] [{}] {}",
        match record.level() {
            Level::Error | Level::Warn => Paint::red(record.level()),
            _ => Paint::white(record.level()),
        },
        match record.target() {
            "UI" => Paint::red("UI"),
            "sim" => Paint::green("sim"),
            "map" => Paint::blue("map"),
            x => Paint::cyan(x),
        },
        record.args()
    )
}

// - If it doesn't make sense to plumb Timer to a library call, return Warn<T>.
// - If there's no Timer, plumb the Warn<T>.
// - If a Timer is available and there's a Warn<T>, use get() or with_context().
// - If a Timer is available and something goes wrong, directly call warn().
// - DO NOT prefer plumbing the Warn<T> and accumulating context. It's usually too tedious. Check
//   out DrawIntersection for an example.
pub struct Warn<T> {
    value: T,
    warnings: Vec<String>,
}

impl<T> Warn<T> {
    pub fn ok(value: T) -> Warn<T> {
        Warn {
            value,
            warnings: Vec::new(),
        }
    }

    pub fn warn(value: T, warning: String) -> Warn<T> {
        Warn {
            value,
            warnings: vec![warning],
        }
    }

    pub fn warnings(value: T, warnings: Vec<String>) -> Warn<T> {
        Warn { value, warnings }
    }

    pub fn unwrap(self) -> T {
        if !self.warnings.is_empty() {
            println!("{} warnings:", self.warnings.len());
            for line in self.warnings {
                println!("{}", line);
            }
        }
        self.value
    }

    pub fn get(self, timer: &mut Timer) -> T {
        // TODO Context from the current Timer phase, caller
        for line in self.warnings {
            timer.warn(line);
        }
        self.value
    }

    pub fn with_context(self, timer: &mut Timer, context: String) -> T {
        for line in self.warnings {
            timer.warn(format!("{}: {}", context, line));
        }
        self.value
    }

    pub fn map<O, F: Fn(T) -> O>(self, f: F) -> Warn<O> {
        Warn {
            value: f(self.value),
            warnings: self.warnings,
        }
    }
}

impl Warn<()> {
    pub fn empty_warnings(warnings: Vec<String>) -> Warn<()> {
        Warn::warnings((), warnings)
    }
}

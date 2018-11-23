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

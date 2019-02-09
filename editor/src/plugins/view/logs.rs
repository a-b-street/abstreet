use crate::objects::DrawCtx;
use crate::plugins::{BlockingPlugin, PluginCtx};
use abstutil::format_log_record;
use ezgui::{GfxCtx, LogScroller};
use lazy_static::lazy_static;
use log::{set_logger, set_max_level, LevelFilter, Log, Metadata, Record};
use std::sync::{Mutex, Once};

lazy_static! {
    static ref LOGGER: Mutex<LogScroller> = Mutex::new(LogScroller::new_with_capacity(100));
}

static START_LOGGER: Once = Once::new();
static LOG_ADAPTER: LogAdapter = LogAdapter;

pub struct DisplayLogs;

impl DisplayLogs {
    pub fn initialize() {
        START_LOGGER.call_once(|| {
            set_max_level(LevelFilter::Debug);
            set_logger(&LOG_ADAPTER).unwrap();
        });
    }

    pub fn new(ctx: &mut PluginCtx) -> Option<DisplayLogs> {
        if ctx.input.action_chosen("show log console") {
            return Some(DisplayLogs);
        }
        None
    }
}

impl BlockingPlugin for DisplayLogs {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if ctx.input.action_chosen("show log console") || LOGGER.lock().unwrap().event(ctx.input) {
            return false;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &DrawCtx) {
        LOGGER.lock().unwrap().draw(g);
    }
}

struct LogAdapter;

impl Log for LogAdapter {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        println!("{}", format_log_record(record));

        // TODO could handle newlines here
        LOGGER.lock().unwrap().add_line(&format!(
            "[{}] [{}] {}",
            record.level(),
            record.target(),
            record.args()
        ));
    }

    fn flush(&self) {}
}

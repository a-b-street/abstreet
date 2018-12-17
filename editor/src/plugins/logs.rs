use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
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

pub struct DisplayLogs {
    active: bool,
}

impl DisplayLogs {
    pub fn new() -> DisplayLogs {
        // Even when the rest of the UI is ripped out, retain this static state.
        START_LOGGER.call_once(|| {
            set_max_level(LevelFilter::Debug);
            set_logger(&LOG_ADAPTER).unwrap();
        });
        DisplayLogs { active: false }
    }
}

impl Plugin for DisplayLogs {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if !self.active {
            if ctx.input.action_chosen("show log console") {
                self.active = true;
                return true;
            } else {
                return false;
            }
        }

        if LOGGER.lock().unwrap().event(ctx.input) {
            self.active = false;
        }
        self.active
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if self.active {
            LOGGER.lock().unwrap().draw(g, ctx.canvas);
        }
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

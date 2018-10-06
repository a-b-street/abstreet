use ezgui::{Canvas, GfxCtx, LogScroller, UserInput};
use log;
use log::{LevelFilter, Log, Metadata, Record};
use objects::ROOT_MENU;
use piston::input::Key;
use plugins::Colorizer;
use std::sync::Mutex;

lazy_static! {
    static ref LOGGER: Mutex<LogScroller> = Mutex::new(LogScroller::new_with_capacity(100));
    static ref LOGGER_STARTED: Mutex<bool> = Mutex::new(false);
}

static LOG_ADAPTER: LogAdapter = LogAdapter;

pub struct DisplayLogs {
    active: bool,
}

impl DisplayLogs {
    pub fn new() -> DisplayLogs {
        // Even when the rest of the UI is ripped out, retain this static state.
        let mut lock = LOGGER_STARTED.lock().unwrap();
        if !*lock {
            log::set_max_level(LevelFilter::Info);
            log::set_logger(&LOG_ADAPTER).unwrap();
            *lock = true;
        }
        DisplayLogs { active: false }
    }

    pub fn event(&mut self, input: &mut UserInput) -> bool {
        if !self.active {
            if input.unimportant_key_pressed(Key::Comma, ROOT_MENU, "show logs") {
                self.active = true;
                return true;
            } else {
                return false;
            }
        }

        if LOGGER.lock().unwrap().event(input) {
            self.active = false;
        }
        self.active
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        if self.active {
            LOGGER.lock().unwrap().draw(g, canvas);
        }
    }
}

impl Colorizer for DisplayLogs {}

struct LogAdapter;

impl Log for LogAdapter {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        use yansi::Paint;

        let line = format!(
            "[{}] [{}] {}",
            Paint::white(record.level()),
            match record.target() {
                "UI" => Paint::red("UI"),
                "sim" => Paint::green("sim"),
                "map" => Paint::blue("map"),
                x => Paint::cyan(x),
            },
            record.args()
        );
        println!("{}", line);
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

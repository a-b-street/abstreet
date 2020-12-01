use std::sync::RwLock;

use instant::Instant;

use crate::elapsed_seconds;

pub struct Logger {
    last_fast_paths_note: RwLock<Option<Instant>>,
}

impl Logger {
    /// On native: intercept messages using the `log` crate and print them to STDOUT. Contains
    /// special handling to filter/throttle spammy messages from `fast_paths` and `hyper`.
    ///
    /// On web: Just use console_log.
    pub fn setup() {
        #[cfg(target_arch = "wasm32")]
        {
            console_log::init_with_level(log::Level::Info).unwrap();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            log::set_boxed_logger(Box::new(Logger {
                last_fast_paths_note: RwLock::new(None),
            }))
            .unwrap();
            log::set_max_level(log::LevelFilter::Info);
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let target = if record.target().len() > 0 {
            record.target()
        } else {
            record.module_path().unwrap_or_default()
        };

        // Throttle these. Only triggered by the importer, by way of map_model
        if target == "fast_paths::fast_graph_builder" {
            let mut last = self.last_fast_paths_note.write().unwrap();
            if last
                .map(|start| elapsed_seconds(start) < 1.0)
                .unwrap_or(false)
            {
                return;
            }
            *last = Some(Instant::now());
        }

        let contents = format!("{}", record.args());

        // Silence these; they're expected on any map using simplified Chinese or kanji. Triggered
        // by anything using widgetry.
        if target == "usvg::convert::text::shaper" && contents.contains("Fallback") {
            return;
        }

        // Silence byte counts from hyper.
        if target == "hyper::proto::h1::io" {
            return;
        }

        println!("[{}] {}: {}", record.level(), target, contents);
    }

    fn flush(&self) {}
}

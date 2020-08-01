use crate::elapsed_seconds;
use instant::Instant;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

pub struct CmdArgs {
    kv: HashMap<String, String>,
    bits: HashSet<String>,
    free: Vec<String>,

    used: HashSet<String>,
}

impl CmdArgs {
    pub fn new() -> CmdArgs {
        // TODO Hijacking this to also initialize logging!
        log::set_boxed_logger(Box::new(Logger {
            last_fp_note: RwLock::new(None),
        }))
        .unwrap();
        log::set_max_level(log::LevelFilter::Trace);

        let mut args = CmdArgs {
            kv: HashMap::new(),
            bits: HashSet::new(),
            free: Vec::new(),
            used: HashSet::new(),
        };

        for arg in std::env::args().skip(1) {
            let parts: Vec<&str> = arg.split('=').collect();
            if parts.len() == 1 {
                if arg.starts_with("--") {
                    args.bits.insert(arg);
                } else {
                    args.free.push(arg);
                }
            } else if parts.len() == 2 {
                args.kv.insert(parts[0].to_string(), parts[1].to_string());
            } else {
                panic!("Weird argument {}", arg);
            }
        }

        args
    }

    pub fn required(&mut self, key: &str) -> String {
        if let Some(value) = self.kv.remove(key) {
            value
        } else {
            panic!("Missing required arg {}", key);
        }
    }

    pub fn optional(&mut self, key: &str) -> Option<String> {
        if let Some(value) = self.kv.remove(key) {
            self.used.insert(key.to_string());
            Some(value)
        } else if self.used.contains(key) {
            panic!("args.optional(\"{}\") called twice!", key);
        } else {
            None
        }
    }

    pub fn optional_parse<T, E, F: Fn(&str) -> Result<T, E>>(
        &mut self,
        key: &str,
        parser: F,
    ) -> Option<T> {
        let value = self.optional(key)?;
        match parser(&value) {
            Ok(result) => Some(result),
            Err(_) => panic!("Bad argument {}={}", key, value),
        }
    }

    pub fn true_false(&mut self, key: &str) -> bool {
        match self.required(key).as_ref() {
            "true" => true,
            "false" => false,
            x => panic!("{}={} is invalid; must be true or false", key, x),
        }
    }

    pub fn enabled(&mut self, key: &str) -> bool {
        if self.bits.remove(key) {
            self.used.insert(key.to_string());
            true
        } else if self.used.contains(key) {
            panic!("args.enabled(\"{}\") called twice!", key);
        } else {
            false
        }
    }

    pub fn required_free(&mut self) -> String {
        if self.free.is_empty() {
            panic!("Required free argument not provided");
        }
        self.free.remove(0)
    }

    pub fn optional_free(&mut self) -> Option<String> {
        if self.free.is_empty() {
            None
        } else {
            Some(self.free.remove(0))
        }
    }

    // TODO Drop?
    pub fn done(&mut self) {
        if !self.kv.is_empty() {
            panic!("Unused arguments: {:?}", self.kv);
        }
        if !self.bits.is_empty() {
            panic!("Unused arguments: {:?}", self.bits);
        }
        if !self.free.is_empty() {
            panic!("Unused free arguments: {:?}", self.free);
        }
    }
}

// TODO Tie this to a Timer
struct Logger {
    last_fp_note: RwLock<Option<Instant>>,
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

        if target == "fast_paths::fast_graph_builder" {
            // Throttle these
            let mut last = self.last_fp_note.write().unwrap();
            if last
                .map(|start| elapsed_seconds(start) < 1.0)
                .unwrap_or(false)
            {
                return;
            }
            *last = Some(Instant::now());
        }
        println!("[{}] {}: {}", record.level(), target, record.args());
    }
    fn flush(&self) {}
}

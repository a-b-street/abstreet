use std::collections::{HashMap, HashSet};

pub struct CmdArgs {
    kv: HashMap<String, String>,
    bits: HashSet<String>,
    free: Vec<String>,
}

impl CmdArgs {
    pub fn new() -> CmdArgs {
        let mut args = CmdArgs {
            kv: HashMap::new(),
            bits: HashSet::new(),
            free: Vec::new(),
        };

        for arg in std::env::args().skip(1) {
            let parts: Vec<&str> = arg.split('=').collect();
            if parts.len() == 1 {
                if arg.starts_with("--") {
                    args.bits.insert(arg[2..].to_string());
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
        self.kv.remove(key)
    }

    pub fn enabled(&mut self, key: &str) -> bool {
        self.bits.remove(key)
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

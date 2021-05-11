use std::collections::{HashMap, HashSet};

/// Yet another barebones command-line flag parsing library.
pub struct CmdArgs {
    kv: HashMap<String, String>,
    bits: HashSet<String>,
    free: Vec<String>,

    used: HashSet<String>,
}

impl CmdArgs {
    /// On native, initialize with real flags. On web, transform URL query parameters into flags.
    ///
    /// Calling this has the side-effect of initializing logging on both native and web. This
    /// should probably be done independently, but for the moment, every app wants both.
    pub fn new() -> CmdArgs {
        crate::logger::setup();

        if cfg!(target_arch = "wasm32") {
            let raw = match parse_args() {
                Ok(raw) => raw,
                Err(err) => {
                    log::warn!("Didn't parse arguments from URL query params: {}", err);
                    Vec::new()
                }
            };
            CmdArgs::from_args(raw)
        } else {
            CmdArgs::from_args(std::env::args().skip(1).collect())
        }
    }

    fn from_args(raw: Vec<String>) -> CmdArgs {
        let mut args = CmdArgs {
            kv: HashMap::new(),
            bits: HashSet::new(),
            free: Vec::new(),
            used: HashSet::new(),
        };

        for arg in raw {
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

impl Default for CmdArgs {
    fn default() -> Self {
        CmdArgs::new()
    }
}

#[cfg(target_arch = "wasm32")]
/// Transform the URL query parameters into command-line arguments, by treating & as the separator
/// between arguments. So for instance "?--dev&--color_scheme=night%20mode" becomes vec!["--dev",
/// "--color_scheme=night mode"].
fn parse_args() -> anyhow::Result<Vec<String>> {
    use anyhow::{anyhow, bail};

    let window = web_sys::window().ok_or(anyhow!("no window?"))?;
    let url = window.location().href().map_err(|err| {
        anyhow!(err
            .as_string()
            .unwrap_or("window.location.href failed".to_string()))
    })?;
    // Consider using a proper url parsing crate. This works fine for now, though.
    let url_parts = url.split("?").collect::<Vec<_>>();
    if url_parts.len() != 2 {
        bail!("URL {} doesn't seem to have query params");
    }
    let parts = url_parts[1]
        .split("&")
        .map(|x| x.replace("%20", " ").to_string())
        .collect::<Vec<_>>();
    Ok(parts)
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_args() -> anyhow::Result<Vec<String>> {
    unreachable!()
}

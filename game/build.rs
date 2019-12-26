use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use walkdir::WalkDir;

// TODO See https://github.com/dtolnay/inventory for an alternate approach.
fn main() {
    let mut mapping: BTreeMap<String, String> = BTreeMap::new();
    for entry in WalkDir::new("src") {
        let path = entry.unwrap().into_path();
        if path.extension() == Some(OsStr::new("rs")) && path != PathBuf::from("src/helpers.rs") {
            for (k, v) in read_file(&format!("{}", path.display())) {
                if mapping.contains_key(&k) {
                    panic!("Color {} defined twice", k);
                }
                mapping.insert(k, v);
            }
        }
    }

    let mut f = File::create(format!("{}/init_colors.rs", env::var("OUT_DIR").unwrap())).unwrap();
    writeln!(f, "fn default_colors() -> HashMap<String, Color> {{").unwrap();
    writeln!(f, "    let mut m = HashMap::new();").unwrap();
    for (k, v) in mapping {
        writeln!(f, "    m.insert(\"{}\".to_string(), {});", k, v).unwrap();
    }
    writeln!(f, "    m").unwrap();
    writeln!(f, "}}").unwrap();
}

fn read_file(path: &str) -> Vec<(String, String)> {
    let mut src = {
        let mut s = String::new();
        let mut f = File::open(path).unwrap();
        f.read_to_string(&mut s).unwrap();
        s
    };

    let mut entries: Vec<(String, String)> = Vec::new();

    while !src.is_empty() {
        if src.starts_with("get_def(") {
            src = src["get_def(".len()..].to_string();

            // Look for the opening "
            while !src.starts_with("\"") {
                src = src[1..].to_string();
            }
            src = src[1..].to_string();

            // Read the key until the closing "
            let mut key = String::new();
            while !src.starts_with("\"") {
                key.push(src.chars().next().unwrap());
                src = src[1..].to_string();
            }
            src = src[1..].to_string();

            // Look for the ,
            while !src.starts_with(",") {
                src = src[1..].to_string();
            }
            src = src[1..].to_string();

            // Look for the Color
            while !src.starts_with("Color") {
                src = src[1..].to_string();
            }

            // Wait for the ()'s to be mismatched, meaning we found the ) of the get_def()
            let mut counter = 0;
            let mut value = String::new();
            loop {
                value.push(src.chars().next().unwrap());
                if src.starts_with("(") {
                    counter += 1;
                } else if src.starts_with(")") {
                    counter -= 1;
                    if counter == -1 {
                        value.pop();
                        entries.push((key, value));
                        src = src[1..].to_string();
                        break;
                    }
                } else if src.starts_with(",") && counter == 0 {
                    value.pop();
                    entries.push((key, value));
                    src = src[1..].to_string();
                    break;
                }
                src = src[1..].to_string();
            }
        } else {
            src = src[1..].to_string();
        }
    }

    entries
}

use std::collections::BTreeSet;
use std::fmt::Write;

pub fn clamp(x: f64, min: f64, max: f64) -> f64 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

pub fn plain_list_names(names: BTreeSet<String>) -> String {
    let mut s = String::new();
    let len = names.len();
    for (idx, n) in names.into_iter().enumerate() {
        if idx != 0 {
            if idx == len - 1 {
                if len == 2 {
                    write!(s, " and ").unwrap();
                } else {
                    write!(s, ", and ").unwrap();
                }
            } else {
                write!(s, ", ").unwrap();
            }
        }
        write!(s, "{}", n).unwrap();
    }
    s
}

pub fn prettyprint_usize(x: usize) -> String {
    let num = format!("{}", x);
    let mut result = String::new();
    let mut i = num.len();
    for c in num.chars() {
        result.push(c);
        i -= 1;
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
    }
    result
}

pub fn basename(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .unwrap()
        .to_os_string()
        .into_string()
        .unwrap()
}

pub fn parent_path(path: &str) -> String {
    format!("{}", std::path::Path::new(path).parent().unwrap().display())
}

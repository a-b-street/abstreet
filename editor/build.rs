use std::env;
use std::path::Path;
use std::process::Command;

// TODO See https://github.com/dtolnay/inventory for an alternate approach.
fn main() {
    let output = Path::new(&env::var("OUT_DIR").unwrap()).join("init_colors.rs");

    assert!(Command::new("/usr/bin/python2")
        .args(&["extract_colorscheme.py", output.to_str().unwrap()])
        .status()
        .unwrap()
        .success());
}

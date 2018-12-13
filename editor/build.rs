use std::process::Command;

fn main() {
    assert!(Command::new("/usr/bin/python2")
        .args(&["extract_colorscheme.py"])
        .status()
        .unwrap()
        .success());
}

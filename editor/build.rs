use std::process::Command;

fn main() {
    // TODO Argh, this runs even when nothing in the editor crate has changed! Constant
    // recompilation. :(
    assert!(Command::new("/usr/bin/python2")
        .args(&["extract_colorscheme.py"])
        .status()
        .unwrap()
        .success());
}

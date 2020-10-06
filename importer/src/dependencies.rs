use std::ffi::OsStr;
use std::process::Command;

use crate::configuration::ImporterConfiguration;

pub fn are_dependencies_callable(config: &ImporterConfiguration) -> bool {
    let mut result = true;

    for command in [
        &config.curl,
        &config.osmconvert,
        &config.unzip,
        &config.gunzip,
    ]
    .iter()
    {
        println!("- Testing if {} is callable", command);
        if !is_program_callable(command) {
            println!("Failed to run {}", command);
            result = false;
        }
    }
    return result;
}

fn is_program_callable<S: AsRef<OsStr>>(name: S) -> bool {
    let output = Command::new(name)
        .arg("-h") // most command line programs return 0 with -h option
        .output(); // suppress output
    output.is_ok()
}

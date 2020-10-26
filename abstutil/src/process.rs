use std::process::Command;

/// Runs a command, asserts success. STDOUT and STDERR aren't touched.
pub fn must_run_cmd(cmd: &mut Command) {
    println!("- Running {:?}", cmd);
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                panic!("{:?} failed", cmd);
            }
        }
        Err(err) => {
            panic!("Failed to run {:?}: {:?}", cmd, err);
        }
    }
}

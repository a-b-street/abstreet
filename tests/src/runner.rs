// https://github.com/rust-lang/rust/issues/50297 would hopefully obsolete this approach.

use abstutil;
use gag::Redirect;
use std;
use yansi::Paint;
use std::io::Write;

pub struct TestRunner {
    results: Vec<TestResult>,
}

struct TestResult {
    test_name: String,
    pass: bool,
    duration: String,
    output_path: String,
}

impl TestRunner {
    pub fn new() -> TestRunner {
        TestRunner {
            results: Vec::new(),
        }
    }

    pub fn run(&mut self, test_name: &str, test: Box<Fn(&mut TestHelper)>) {
        print!("Running {}...", test_name);
        std::io::stdout().flush().unwrap();

        // TODO Make a temporary directory inside /tmp, remove successful files
        let start = std::time::Instant::now();
        let mut helper = TestHelper {};
        let output_path = format!("/tmp/{}.log", test_name);
        std::fs::create_dir_all(std::path::Path::new(&output_path).parent().unwrap())
            .expect("Creating parent dir failed");

        let pass = {
            let _stdout_redirect = Redirect::stdout(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output_path.clone())
                    .unwrap(),
            ).unwrap();
            let _stderr_redirect = Redirect::stderr(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output_path.clone())
                    .unwrap(),
            ).unwrap();

            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                test(&mut helper);
            })).is_ok()
        };
        let duration = format!("{:.02}s", abstutil::elapsed_seconds(start));
        print!("\rRunning {}... {}\n", test_name, duration);
        self.results.push(TestResult {
            test_name: test_name.to_string(),
            pass,
            duration,
            output_path,
        });
    }

    pub fn done(self) {
        println!("");
        let mut passed = 0;
        let mut failed = 0;
        for result in self.results.into_iter() {
            if result.pass {
                passed += 1;
                println!("- {} ({}): {}", result.test_name, result.duration, Paint::green("PASS"));
            } else {
                failed += 1;
                println!("- {} ({}): {}", result.test_name, result.duration, Paint::red("FAIL"));
                println!("    {}", Paint::cyan(result.output_path));
            }
        }

        println!("{} tests passed, {} tests failed", passed, failed);
    }
}

pub struct TestHelper {}

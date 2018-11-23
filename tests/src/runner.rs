// https://github.com/rust-lang/rust/issues/50297 would hopefully obsolete this approach.

use gag::Redirect;
use std;
use yansi::Paint;

pub struct TestRunner {
    results: Vec<TestResult>,
}

struct TestResult {
    test_name: String,
    pass: bool,
    output_path: String,
}

impl TestRunner {
    pub fn new() -> TestRunner {
        TestRunner {
            results: Vec::new(),
        }
    }

    pub fn run(&mut self, test_name: &str, test: Box<Fn(&mut TestHelper)>) {
        // TODO Make a temporary directory inside /tmp, remove successful files
        let output_path = format!("/tmp/{}.log", test_name);
        std::fs::create_dir_all(std::path::Path::new(&output_path).parent().unwrap())
            .expect("Creating parent dir failed");
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

        let mut helper = TestHelper {};

        let pass = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            test(&mut helper);
        })).is_ok();
        self.results.push(TestResult {
            test_name: test_name.to_string(),
            pass,
            output_path,
        });
    }

    pub fn done(self) {
        let mut passed = 0;
        let mut failed = 0;
        for result in self.results.into_iter() {
            if result.pass {
                passed += 1;
                println!("- {}: {}", result.test_name, Paint::green("PASS"));
            } else {
                failed += 1;
                println!("- {}: {}", result.test_name, Paint::red("FAIL"));
                println!("    {}", Paint::cyan(result.output_path));
            }
        }

        println!("{} tests passed, {} tests failed", passed, failed);
    }
}

pub struct TestHelper {}

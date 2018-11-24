// https://github.com/rust-lang/rust/issues/50297 would hopefully obsolete this approach.

use abstutil;
use abstutil::Error;
use gag::Redirect;
use std;
use std::io::Write;
use yansi::Paint;

pub struct TestRunner {
    current_suite: Option<String>,
    results: Vec<TestResult>,
    filter: Filter,
    test_name_filter: Option<String>,
    output_dir: String,
}

struct TestResult {
    test_name: String,
    pass: bool,
    duration: String,
    output_path: String,
}

impl TestRunner {
    pub fn new(filter: Filter, test_name_filter: Option<String>) -> TestRunner {
        TestRunner {
            current_suite: None,
            results: Vec::new(),
            filter,
            test_name_filter,
            output_dir: format!(
                "/tmp/abst_tests_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
        }
    }

    pub fn suite(&mut self, name: &str) -> &mut TestRunner {
        self.current_suite = Some(name.to_string());
        self
    }

    pub fn run_fast(&mut self, specific_test_name: &str, test: Box<Fn(&mut TestHelper)>) {
        self.run(specific_test_name, true, test);
    }

    pub fn run_slow(&mut self, specific_test_name: &str, test: Box<Fn(&mut TestHelper)>) {
        self.run(specific_test_name, false, test);
    }

    fn run(&mut self, specific_test_name: &str, fast: bool, test: Box<Fn(&mut TestHelper)>) {
        let test_name = format!(
            "{}/{}",
            self.current_suite
                .as_ref()
                .expect("Can't run() a test without suite()"),
            specific_test_name
        );

        if (fast && self.filter == Filter::Slow) || (!fast && self.filter == Filter::Fast) {
            return;
        }
        if let Some(ref filter) = self.test_name_filter {
            if !test_name.contains(filter) {
                return;
            }
        }

        print!("Running {}...", test_name);
        std::io::stdout().flush().unwrap();

        let start = std::time::Instant::now();
        let mut helper = TestHelper {};
        let output_path = format!("{}/{}.log", self.output_dir, test_name);
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
        if pass {
            print!(
                "\rRunning {}... {} in {}\n",
                test_name,
                Paint::green("PASS"),
                duration
            );
            std::fs::remove_file(&output_path).expect(&format!(
                "Couldn't delete successful test log {}",
                output_path
            ));
        } else {
            print!(
                "\rRunning {}... {} in {}\n",
                test_name,
                Paint::red("FAIL"),
                duration
            );
            println!("  {}", Paint::cyan(&output_path));
        }

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
                println!(
                    "- {} ({}): {}",
                    result.test_name,
                    result.duration,
                    Paint::green("PASS")
                );
            } else {
                failed += 1;
                println!(
                    "- {} ({}): {}",
                    result.test_name,
                    result.duration,
                    Paint::red("FAIL")
                );
                println!("    {}", Paint::cyan(result.output_path));
            }
        }

        println!("{} tests passed, {} tests failed", passed, failed);
    }
}

pub struct TestHelper {}

#[derive(PartialEq)]
pub enum Filter {
    All,
    Slow,
    Fast,
}

impl std::str::FromStr for Filter {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "All" => Ok(Filter::All),
            "Slow" => Ok(Filter::Slow),
            "Fast" => Ok(Filter::Fast),
            _ => Err(Error::new(format!("{} isn't a valid Filter", s))),
        }
    }
}

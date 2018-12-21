// https://github.com/rust-lang/rust/issues/50297 would hopefully obsolete this approach.

use abstutil;
use abstutil::Error;
use gag::Redirect;
use sim::Sim;
use std;
use std::io::Write;
use structopt::StructOpt;
use termion;
use termion::color;

#[derive(StructOpt)]
#[structopt(name = "tests")]
pub struct Flags {
    /// Which tests to run?
    #[structopt(long = "filter", default_value = "All")]
    filter: Filter,

    /// If specified, only run tests with names containing this substring.
    #[structopt(long = "test_names")]
    test_names: Option<String>,

    /// Keep the log and savestate even for passing tests.
    #[structopt(long = "keep_output")]
    keep_output: bool,

    /// Print debug output as clickable HTTP links.
    #[structopt(long = "clickable_links")]
    clickable_links: bool,
}

pub struct TestRunner {
    current_suite: Option<String>,
    results: Vec<TestResult>,
    flags: Flags,
    output_dir: String,
    started_at: std::time::Instant,
}

struct TestResult {
    test_name: String,
    pass: bool,
    duration: String,
    output_path: String,
    debug_with_savestate: Option<String>,
}

impl TestResult {
    fn print(&self, flags: &Flags) {
        let reset_color = color::Fg(color::Reset);

        if self.pass {
            println!(
                "- {} ({}): {}PASS{}",
                self.test_name,
                self.duration,
                color::Fg(color::Green),
                reset_color
            );
        } else {
            println!(
                "- {} ({}): {}FAIL{}",
                self.test_name,
                self.duration,
                color::Fg(color::Red),
                reset_color
            );
        }
        if !self.pass || flags.keep_output {
            if flags.clickable_links {
                println!(
                    "    {}http://most/{}{}",
                    color::Fg(color::Cyan),
                    self.output_path,
                    reset_color
                );
            } else {
                println!(
                    "    {}{}{}",
                    color::Fg(color::Cyan),
                    self.output_path,
                    reset_color
                );
            }

            if let Some(ref path) = self.debug_with_savestate {
                if flags.clickable_links {
                    println!(
                        "    {}http://ui/{}{}",
                        color::Fg(color::Yellow),
                        path,
                        reset_color
                    );
                } else {
                    println!("    {}{}{}", color::Fg(color::Yellow), path, reset_color);
                }
            }
        }
    }
}

impl TestRunner {
    pub fn new(flags: Flags) -> TestRunner {
        TestRunner {
            current_suite: None,
            results: Vec::new(),
            flags,
            output_dir: format!(
                "/tmp/abst_tests_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            started_at: std::time::Instant::now(),
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
        let reset_color = color::Fg(color::Reset);

        let test_name = format!(
            "{}/{}",
            self.current_suite
                .as_ref()
                .expect("Can't run() a test without suite()"),
            specific_test_name
        );

        if (fast && self.flags.filter == Filter::Slow)
            || (!fast && self.flags.filter == Filter::Fast)
        {
            return;
        }
        if let Some(ref filter) = self.flags.test_names {
            if !test_name.contains(filter) {
                return;
            }
        }

        let start = std::time::Instant::now();
        let mut helper = TestHelper {
            debug_with_savestate: None,
        };
        let output_path = format!("{}/{}.log", self.output_dir, test_name);
        std::fs::create_dir_all(std::path::Path::new(&output_path).parent().unwrap())
            .expect("Creating parent dir failed");

        let line_width = if self.flags.clickable_links {
            print!(
                "Running {}... {}http://tail/{}{}",
                test_name,
                color::Fg(color::Cyan),
                output_path,
                reset_color
            );
            format!("Running {}... http://tail/{}", test_name, output_path).len()
        } else {
            print!(
                "Running {}... {}tail -f {}{}",
                test_name,
                color::Fg(color::Cyan),
                output_path,
                reset_color
            );
            format!("Running {}... {}", test_name, output_path).len()
        };
        std::io::stdout().flush().unwrap();

        let pass = {
            let _stdout_redirect = Redirect::stdout(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output_path.clone())
                    .unwrap(),
            )
            .unwrap();
            let _stderr_redirect = Redirect::stderr(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output_path.clone())
                    .unwrap(),
            )
            .unwrap();

            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                test(&mut helper);
            }))
            .is_ok()
        };

        if pass && !self.flags.keep_output {
            std::fs::remove_file(&output_path).expect(&format!(
                "Couldn't delete successful test log {}",
                output_path
            ));
        }
        let result = TestResult {
            test_name: test_name.to_string(),
            pass,
            duration: format!("{:.02}s", abstutil::elapsed_seconds(start)),
            output_path,
            debug_with_savestate: helper.debug_with_savestate,
        };

        let (terminal_width, _) = termion::terminal_size().unwrap();
        // TODO Repeat for more than one line up, if needed.
        print!(
            "{}{}",
            termion::clear::CurrentLine,
            termion::cursor::Left(terminal_width)
        );
        if line_width > terminal_width as usize {
            print!("{}{}", termion::cursor::Up(1), termion::clear::CurrentLine);
        }

        result.print(&self.flags);
        self.results.push(result);
    }

    pub fn done(self) {
        let mut passed = 0;
        let mut failed = 0;
        for result in self.results.into_iter() {
            if result.pass {
                passed += 1;
            } else {
                failed += 1;
            }
        }

        println!(
            "\n{} tests passed, {} tests failed in {:.02}s",
            passed,
            failed,
            abstutil::elapsed_seconds(self.started_at)
        );
    }
}

pub struct TestHelper {
    debug_with_savestate: Option<String>,
}

impl TestHelper {
    pub fn setup_done(&mut self, sim: &Sim) {
        if self.debug_with_savestate.is_some() {
            panic!("Can't call setup_done twice in one test");
        }
        self.debug_with_savestate = Some(sim.save());
    }
}

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

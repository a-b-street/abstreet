use std::collections::VecDeque;
use std::time::Duration;

use instant::Instant;
use subprocess::{Communicator, Popen};

use widgetry::{Color, EventCtx, GfxCtx, Line, Panel, State, Text, Transition, UpdateType};

use crate::tools::PopupMsg;
use crate::AppLike;

/// Executes a command and displays STDOUT and STDERR in a loading screen window. Only works on
/// native, of course.
pub struct RunCommand<A: AppLike> {
    p: Popen,
    // Only wrapped in an Option so we can modify it when we're almost done.
    comm: Option<Communicator>,
    panel: Panel,
    lines: VecDeque<String>,
    max_capacity: usize,
    started: Instant,
    last_drawn: Instant,
    // Wrapped in an Option just to make calling from event() work. The bool is success, and the
    // strings are the last lines of output.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A, bool, Vec<String>) -> Transition<A>>>,
}

impl<A: AppLike + 'static> RunCommand<A> {
    pub fn new(
        ctx: &mut EventCtx,
        _: &A,
        args: Vec<String>,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, bool, Vec<String>) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        info!("RunCommand: {}", args.join(" "));
        match subprocess::Popen::create(
            &args,
            subprocess::PopenConfig {
                stdout: subprocess::Redirection::Pipe,
                stderr: subprocess::Redirection::Merge,
                ..Default::default()
            },
        ) {
            Ok(mut p) => {
                let comm = Some(
                    p.communicate_start(None)
                        .limit_time(Duration::from_millis(0)),
                );
                let panel = ctx.make_loading_screen(Text::from("Starting command..."));
                let max_capacity =
                    (0.8 * ctx.canvas.window_height / ctx.default_line_height()) as usize;
                Box::new(RunCommand {
                    p,
                    comm,
                    panel,
                    lines: VecDeque::new(),
                    max_capacity,
                    started: Instant::now(),
                    last_drawn: Instant::now(),
                    on_load: Some(on_load),
                })
            }
            Err(err) => PopupMsg::new(
                ctx,
                "Error",
                vec![format!("Couldn't start command: {}", err)],
            ),
        }
    }

    fn read_output(&mut self) {
        let mut new_lines = Vec::new();
        let (stdout, stderr) = match self.comm.as_mut().unwrap().read() {
            Ok(pair) => pair,
            // This is almost always a timeout.
            Err(err) => err.capture,
        };
        assert!(stderr.is_none());
        if let Some(bytes) = stdout {
            if let Ok(string) = String::from_utf8(bytes) {
                if !string.is_empty() {
                    for line in string.split("\n") {
                        new_lines.push(line.to_string());
                    }
                }
            }
        }
        for line in new_lines {
            if self.lines.len() == self.max_capacity {
                self.lines.pop_front();
            }
            if line.contains("\r") {
                // \r shows up in two cases:
                // 1) As output from docker
                // 2) As the "clear the current line" escape code
                // TODO Assuming always 2 parts...
                let parts = line.split('\r').collect::<Vec<_>>();
                if parts[0].is_empty() {
                    self.lines.pop_back();
                    self.lines.push_back(parts[1].to_string());
                } else {
                    println!("> {}", parts[0]);
                    self.lines.push_back(parts[0].to_string());
                }
            } else {
                println!("> {}", line);
                self.lines.push_back(line);
            }
        }
    }
}

impl<A: AppLike + 'static> State<A> for RunCommand<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        ctx.request_update(UpdateType::Game);
        if ctx.input.nonblocking_is_update_event().is_none() {
            return Transition::Keep;
        }

        self.read_output();

        // Throttle rerendering
        if abstutil::elapsed_seconds(self.last_drawn) > 0.1 {
            let mut txt = Text::from(
                Line(format!(
                    "Running command... {} so far",
                    geom::Duration::realtime_elapsed(self.started)
                ))
                .small_heading(),
            );
            for line in &self.lines {
                txt.add_line(line);
            }
            self.panel = ctx.make_loading_screen(txt);
            self.last_drawn = Instant::now();
        }

        if let Some(status) = self.p.poll() {
            // Make sure to grab all remaining output.
            let comm = self.comm.take().unwrap();
            self.comm = Some(comm.limit_time(Duration::from_secs(10)));
            self.read_output();
            // TODO Possible hack -- why is this last line empty?
            if self.lines.back().map(|x| x.is_empty()).unwrap_or(false) {
                self.lines.pop_back();
            }

            let success = status.success();
            let mut lines: Vec<String> = self.lines.drain(..).collect();
            if !success {
                lines.push(format!("Command failed: {:?}", status));
            }
            return Transition::Multi(vec![
                Transition::Pop,
                (self.on_load.take().unwrap())(ctx, app, success, lines.clone()),
                Transition::Push(PopupMsg::new(
                    ctx,
                    if success { "Success!" } else { "Failure!" },
                    lines,
                )),
            ]);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        g.clear(Color::BLACK);
        self.panel.draw(g);
    }
}

use crate::runner::{State, GUI};
use crate::{Canvas, GfxCtx};
use abstutil::Timer;
use std::io::Write;
use std::{fs, panic, process};

pub(crate) fn screenshot_everything<T, G: GUI<T>>(
    mut state: State<T, G>,
    display: &glium::Display,
    program: &glium::Program,
    zoom: f64,
    max_x: f64,
    max_y: f64,
) -> State<T, G> {
    // TODO Reorganize internals too.
    let capture = ScreenCaptureState::new(&mut state.canvas, zoom, max_x, max_y);
    capture.run(state, display, program)
}

struct ScreenCaptureState {
    timer: Timer,
    filenames: Vec<String>,

    num_tiles_x: usize,
    num_tiles_y: usize,
    orig_zoom: f64,
    orig_x: f64,
    orig_y: f64,
}

impl ScreenCaptureState {
    fn new(canvas: &mut Canvas, zoom: f64, max_x: f64, max_y: f64) -> ScreenCaptureState {
        let num_tiles_x = (max_x * zoom / canvas.window_width).floor() as usize;
        let num_tiles_y = (max_y * zoom / canvas.window_height).floor() as usize;
        let mut timer = Timer::new("capturing screen");
        timer.start_iter("capturing images", num_tiles_x * num_tiles_y);
        fs::create_dir("screencap").unwrap();
        let state = ScreenCaptureState {
            timer,
            filenames: Vec::new(),
            num_tiles_x,
            num_tiles_y,
            orig_zoom: canvas.cam_zoom,
            orig_x: canvas.cam_x,
            orig_y: canvas.cam_y,
        };
        canvas.cam_zoom = zoom;
        state
    }

    fn run<T, G: GUI<T>>(
        mut self,
        mut state: State<T, G>,
        display: &glium::Display,
        program: &glium::Program,
    ) -> State<T, G> {
        let last_data = state.last_data.as_ref().unwrap();

        for tile_y in 0..self.num_tiles_y {
            for tile_x in 0..self.num_tiles_x {
                self.timer.next();
                state.canvas.cam_x = (tile_x as f64) * state.canvas.window_width;
                state.canvas.cam_y = (tile_y as f64) * state.canvas.window_height;

                let mut target = display.draw();
                let mut g = GfxCtx::new(&state.canvas, &display, &mut target, program);

                let gui = state.gui;
                let naming_hint = match panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    gui.new_draw(&mut g, last_data, true)
                })) {
                    Ok(naming_hint) => naming_hint,
                    Err(err) => {
                        gui.dump_before_abort(&state.canvas);
                        panic::resume_unwind(err);
                    }
                };
                state.gui = gui;
                target.finish().unwrap();

                if !self.screencap(tile_x, tile_y, naming_hint) {
                    return state;
                }
            }
        }

        state.canvas.cam_zoom = self.orig_zoom;
        state.canvas.cam_x = self.orig_x;
        state.canvas.cam_y = self.orig_y;
        self.finish();

        state
    }

    fn screencap(&mut self, tile_x: usize, tile_y: usize, mut naming_hint: Option<String>) -> bool {
        let suffix = naming_hint.take().unwrap_or_else(String::new);
        let filename = format!("{:02}x{:02}{}.png", tile_x + 1, tile_y + 1, suffix);
        if !process::Command::new("scrot")
            .args(&[
                "--quality",
                "100",
                "--focused",
                "--silent",
                &format!("screencap/{}", filename),
            ])
            .status()
            .unwrap()
            .success()
        {
            println!("scrot failed; aborting");
            return false;
        }
        self.filenames.push(filename);
        true
    }

    fn finish(self) {
        let mut args = self.filenames;
        args.push("-mode".to_string());
        args.push("Concatenate".to_string());
        args.push("-tile".to_string());
        args.push(format!("{}x{}", self.num_tiles_x, self.num_tiles_y));
        args.push("full.png".to_string());

        let mut file = fs::File::create("screencap/combine.sh").unwrap();
        writeln!(file, "#!/bin/bash\n").unwrap();
        writeln!(file, "montage {}", args.join(" ")).unwrap();
        writeln!(file, "rm -f combine.sh").unwrap();
    }
}

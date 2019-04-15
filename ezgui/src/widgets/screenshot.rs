use crate::runner::{State, GUI};
use crate::Prerender;
use abstutil::Timer;
use std::io::Write;
use std::{fs, process, thread, time};

pub(crate) fn screenshot_everything<T, G: GUI<T>>(
    dir_path: &str,
    mut state: State<T, G>,
    display: &glium::Display,
    program: &glium::Program,
    prerender: &Prerender,
    zoom: f64,
    max_x: f64,
    max_y: f64,
) -> State<T, G> {
    let mut timer = Timer::new("capturing screen");
    let num_tiles_x = (max_x * zoom / state.canvas.window_width).ceil() as usize;
    let num_tiles_y = (max_y * zoom / state.canvas.window_height).ceil() as usize;
    let orig_zoom = state.canvas.cam_zoom;
    let orig_x = state.canvas.cam_x;
    let orig_y = state.canvas.cam_y;

    timer.start_iter("capturing images", num_tiles_x * num_tiles_y);
    let mut filenames: Vec<String> = Vec::new();
    state.canvas.cam_zoom = zoom;
    fs::create_dir_all(dir_path).unwrap();

    for tile_y in 0..num_tiles_y {
        for tile_x in 0..num_tiles_x {
            timer.next();
            state.canvas.cam_x = (tile_x as f64) * state.canvas.window_width;
            state.canvas.cam_y = (tile_y as f64) * state.canvas.window_height;

            let suffix = state
                .draw(display, program, prerender, true)
                .unwrap_or_else(String::new);
            let filename = format!("{:02}x{:02}{}.png", tile_x + 1, tile_y + 1, suffix);

            // TODO Is vsync or something else causing the above redraw to not actually show up in
            // time for scrot to see it? This is slow (30s total for Montlake), but stable.
            thread::sleep(time::Duration::from_millis(100));

            if screencap(&format!("{}/{}", dir_path, filename)) {
                filenames.push(filename);
            } else {
                // Abort early.
                return state;
            }
        }
    }

    state.canvas.cam_zoom = orig_zoom;
    state.canvas.cam_x = orig_x;
    state.canvas.cam_y = orig_y;
    finish(dir_path, filenames, num_tiles_x, num_tiles_y);

    state
}

pub(crate) fn screenshot_current<T, G: GUI<T>>(
    state: &mut State<T, G>,
    display: &glium::Display,
    program: &glium::Program,
    prerender: &Prerender,
) {
    state.draw(display, program, prerender, true);
    thread::sleep(time::Duration::from_millis(100));
    screencap("screenshot.png");
}

fn screencap(filename: &str) -> bool {
    if !process::Command::new("scrot")
        .args(&["--quality", "100", "--focused", "--silent", filename])
        .status()
        .unwrap()
        .success()
    {
        println!("scrot failed; aborting");
        return false;
    }
    true
}

fn finish(dir_path: &str, filenames: Vec<String>, num_tiles_x: usize, num_tiles_y: usize) {
    {
        let mut args = filenames.clone();
        args.push("-mode".to_string());
        args.push("Concatenate".to_string());
        args.push("-tile".to_string());
        args.push(format!("{}x{}", num_tiles_x, num_tiles_y));
        args.push("full.png".to_string());

        let mut file = fs::File::create(format!("{}/combine.sh", dir_path)).unwrap();
        writeln!(file, "#!/bin/bash\n").unwrap();
        writeln!(file, "montage {}", args.join(" ")).unwrap();
        writeln!(file, "rm -f combine.sh").unwrap();
    }

    {
        let output = process::Command::new("md5sum")
            .args(
                &filenames
                    .into_iter()
                    .map(|f| format!("{}/{}", dir_path, f))
                    .collect::<Vec<String>>(),
            )
            .output()
            .unwrap();
        assert!(output.status.success());
        let mut file = fs::File::create(format!("{}/MANIFEST", dir_path)).unwrap();
        file.write_all(&output.stdout).unwrap();
    }
}

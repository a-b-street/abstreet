use crate::runner::{State, GUI};
use crate::Prerender;
use abstutil::Timer;
use std::io::Write;
use std::{fs, process, thread, time};

pub(crate) fn screenshot_everything<G: GUI>(
    state: &mut State<G>,
    dir_path: &str,
    prerender: &Prerender,
    zoom: f64,
    max_x: f64,
    max_y: f64,
) {
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

            let suffix = state.draw(prerender, true).unwrap_or_else(String::new);
            let filename = format!("{:02}x{:02}{}.gif", tile_x + 1, tile_y + 1, suffix);

            // TODO Is vsync or something else causing the above redraw to not actually show up in
            // time for scrot to see it? This is slow (30s total for Montlake), but stable.
            thread::sleep(time::Duration::from_millis(100));

            if screencap(&format!("{}/{}", dir_path, filename)) {
                filenames.push(filename);
            } else {
                // Abort early.
                return;
            }
        }
    }

    state.canvas.cam_zoom = orig_zoom;
    state.canvas.cam_x = orig_x;
    state.canvas.cam_y = orig_y;
    finish(dir_path, filenames, num_tiles_x, num_tiles_y);
}

fn screencap(filename: &str) -> bool {
    if !process::Command::new("scrot")
        .args(&[
            "--quality",
            "100",
            "--focused",
            "--silent",
            "screenshot.png",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        println!("Screencapping failed; you probably don't have scrot (https://en.wikipedia.org/wiki/Scrot) installed");
        return false;
    }
    if !process::Command::new("convert")
        .arg("screenshot.png")
        .arg(filename)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        println!(
            "Screencapping failed; you probably don't have convert (https://imagemagick.org) \
             installed"
        );
        return false;
    }
    process::Command::new("rm")
        .arg("screenshot.png")
        .status()
        .unwrap();

    true
}

fn finish(dir_path: &str, filenames: Vec<String>, num_tiles_x: usize, num_tiles_y: usize) {
    let mut args = filenames;
    args.push("-mode".to_string());
    args.push("Concatenate".to_string());
    args.push("-tile".to_string());
    args.push(format!("{}x{}", num_tiles_x, num_tiles_y));
    args.push("full.gif".to_string());

    let mut file = fs::File::create(format!("{}/combine.sh", dir_path)).unwrap();
    writeln!(file, "#!/bin/bash\n").unwrap();
    writeln!(file, "montage {}", args.join(" ")).unwrap();
    writeln!(file, "rm -f combine.sh").unwrap();
}

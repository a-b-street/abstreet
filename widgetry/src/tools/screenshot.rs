use abstutil::Timer;

use crate::runner::State;
use crate::{Prerender, ScreenDims, SharedAppState};

/// Take a screenshot of the entire canvas, tiling it based on the window's width and height.
pub(crate) fn screenshot_everything<A: SharedAppState>(
    state: &mut State<A>,
    dir_path: &str,
    prerender: &Prerender,
    zoom: f64,
    dims: ScreenDims,
    leaflet_naming: bool,
) -> anyhow::Result<()> {
    if dims.width > state.canvas.window_width || dims.height > state.canvas.window_height {
        bail!(
            "Can't take screenshots of dims {:?} when the window is only {:?}",
            dims,
            state.canvas.get_window_dims()
        );
    }

    let mut timer = Timer::new("capturing screen");
    let num_tiles_x = (state.canvas.map_dims.0 * zoom / dims.width).ceil() as usize;
    let num_tiles_y = (state.canvas.map_dims.1 * zoom / dims.height).ceil() as usize;
    let orig_zoom = state.canvas.cam_zoom;
    let orig_x = state.canvas.cam_x;
    let orig_y = state.canvas.cam_y;

    timer.start_iter("capturing images", num_tiles_x * num_tiles_y);
    state.canvas.cam_zoom = zoom;
    std::fs::create_dir_all(dir_path)?;

    for tile_y in 0..num_tiles_y {
        for tile_x in 0..num_tiles_x {
            timer.next();
            state.canvas.cam_x = (tile_x as f64) * dims.width;
            state.canvas.cam_y = (tile_y as f64) * dims.height;

            let suffix = state.draw(prerender, true).unwrap_or_else(String::new);

            // Sometimes the very first image captured is of the debug mode used to launch this. Not
            // sure why, and it's not so reproducible. But double-drawing seems to help.
            if tile_x == 0 && tile_y == 0 {
                state.draw(prerender, true);
            }

            let filename = if leaflet_naming {
                format!("{}/{}_{}.png", dir_path, tile_x, tile_y)
            } else {
                format!(
                    "{}/{:02}x{:02}{}.png",
                    dir_path,
                    tile_x + 1,
                    tile_y + 1,
                    suffix
                )
            };
            prerender.inner.screencap(dims, filename.clone())?;
        }
    }

    state.canvas.cam_zoom = orig_zoom;
    state.canvas.cam_x = orig_x;
    state.canvas.cam_y = orig_y;
    Ok(())
}

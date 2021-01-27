use serde::{Deserialize, Serialize};
use widgetry::{Canvas, EventCtx};

use abstio::MapName;
use abstutil::Timer;

/// Represents the state of a widgetry Canvas.
#[derive(Serialize, Deserialize, Debug)]
pub struct CameraState {
    cam_x: f64,
    cam_y: f64,
    cam_zoom: f64,
}

impl CameraState {
    /// Save the camera's configuration for the specified map.
    pub fn save(canvas: &Canvas, name: &MapName) {
        let state = CameraState {
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        abstio::write_json(abstio::path_camera_state(name), &state);
    }

    /// Load the camera's configuration for the specified map. Returns true if successful, has no
    /// effect if the file is missing or broken.
    pub fn load(ctx: &mut EventCtx, name: &MapName) -> bool {
        match abstio::maybe_read_json::<CameraState>(
            abstio::path_camera_state(name),
            &mut Timer::throwaway(),
        ) {
            Ok(ref loaded) => {
                ctx.canvas.cam_x = loaded.cam_x;
                ctx.canvas.cam_y = loaded.cam_y;
                ctx.canvas.cam_zoom = loaded.cam_zoom;
                true
            }
            Err(_) => false,
        }
    }
}

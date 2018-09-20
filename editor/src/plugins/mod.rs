pub mod classification;
pub mod color_picker;
pub mod debug_objects;
pub mod draw_polygon;
pub mod floodfill;
pub mod follow;
pub mod geom_validation;
pub mod hider;
pub mod road_editor;
pub mod search;
pub mod show_route;
pub mod sim_controls;
pub mod steep;
pub mod stop_sign_editor;
pub mod traffic_signal_editor;
pub mod turn_cycler;
pub mod warp;
pub mod wizard;

use graphics::types::Color;
use objects::{Ctx, ID};

pub trait Colorizer {
    fn color_for(&self, _obj: ID, _ctx: Ctx) -> Option<Color> {
        None
    }
}

pub mod classification;
pub mod color_picker;
pub mod debug_objects;
pub mod draw_neighborhoods;
pub mod floodfill;
pub mod follow;
pub mod geom_validation;
pub mod hider;
pub mod logs;
pub mod map_edits;
pub mod road_editor;
pub mod scenarios;
pub mod search;
pub mod show_route;
pub mod sim_controls;
pub mod steep;
pub mod stop_sign_editor;
pub mod traffic_signal_editor;
pub mod turn_cycler;
pub mod warp;

use ezgui::Color;
use objects::{Ctx, ID};

pub trait Colorizer {
    fn color_for(&self, _obj: ID, _ctx: Ctx) -> Option<Color> {
        None
    }
}

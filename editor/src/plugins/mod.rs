pub mod classification;
pub mod color_picker;
pub mod debug_objects;
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
pub mod turn_colors;
pub mod turn_cycler;
pub mod warp;

use colors::ColorScheme;
use control::ControlMap;
use graphics::types::Color;
use map_model::Map;
use objects::ID;

pub trait Colorizer {
    fn color_for(&self, _obj: ID, _ctx: Ctx) -> Option<Color> {
        None
    }
}

pub struct Ctx<'a> {
    pub cs: &'a ColorScheme,
    pub map: &'a Map,
    pub control_map: &'a ControlMap,
}

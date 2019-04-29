mod area;
mod bike;
mod building;
mod bus_stop;
mod car;
mod extra_shape;
mod intersection;
mod lane;
mod map;
mod pedestrian;
mod road;
mod turn;

use crate::helpers::{ColorScheme, DrawCtx, ID};
pub use crate::render::area::DrawArea;
use crate::render::bike::DrawBike;
use crate::render::car::DrawCar;
pub use crate::render::extra_shape::ExtraShapeID;
pub use crate::render::intersection::{calculate_corners, draw_signal_cycle, draw_signal_diagram};
pub use crate::render::lane::DrawLane;
pub use crate::render::map::{AgentCache, DrawMap};
pub use crate::render::pedestrian::DrawPedestrian;
pub use crate::render::road::DrawRoad;
pub use crate::render::turn::{DrawCrosswalk, DrawTurn};
use ezgui::{Color, GfxCtx, Prerender};
use geom::{Distance, Polygon};
use map_model::Map;
use sim::{DrawCarInput, VehicleType};

pub const MIN_ZOOM_FOR_DETAIL: f64 = 1.0;

const EXTRA_SHAPE_THICKNESS: Distance = Distance::const_meters(1.0);
const EXTRA_SHAPE_POINT_RADIUS: Distance = Distance::const_meters(1.0);

const BIG_ARROW_THICKNESS: Distance = Distance::const_meters(0.5);

const TURN_ICON_ARROW_THICKNESS: Distance = Distance::const_meters(0.15);
const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(2.0);
pub const CROSSWALK_LINE_THICKNESS: Distance = Distance::const_meters(0.25);

// Does something belong here or as a method on ID? If it ONLY applies to renderable things, then
// here. For example, trips aren't drawn, so it's meaningless to ask what their bounding box is.
pub trait Renderable {
    fn get_id(&self) -> ID;
    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &DrawCtx);
    // Higher z-ordered objects are drawn later. Default to low so roads at -1 don't vanish.
    fn get_zorder(&self) -> isize {
        -5
    }
    // This is called at most once per frame; don't worry about cloning and lack of prerendering.
    fn get_outline(&self, map: &Map) -> Polygon;
}

pub struct RenderOptions {
    // The "main" color for the object, if available.
    pub color: Option<Color>,
    // TODO This should be accessible through ctx...
    pub debug_mode: bool,
}

pub fn draw_vehicle(
    input: DrawCarInput,
    map: &Map,
    prerender: &Prerender,
    cs: &ColorScheme,
) -> Box<Renderable> {
    if input.vehicle_type == VehicleType::Bike {
        Box::new(DrawBike::new(input, map, prerender, cs))
    } else {
        Box::new(DrawCar::new(input, map, prerender, cs))
    }
}

// Don't use the simulation time, because then fast simulations would have cars blinking _very_
// fast or slow. Unless that's what people expect?
pub fn should_draw_blinkers() -> bool {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_millis()
        % 300
        < 150
}

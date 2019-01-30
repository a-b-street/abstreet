mod area;
mod bike;
mod building;
mod bus_stop;
mod car;
mod extra_shape;
mod intersection;
mod lane;
mod map;
mod parcel;
mod pedestrian;
mod turn;

use crate::objects::{Ctx, ID};
pub use crate::render::area::DrawArea;
use crate::render::bike::DrawBike;
use crate::render::car::DrawCar;
pub use crate::render::extra_shape::ExtraShapeID;
pub use crate::render::intersection::{draw_signal_cycle, draw_signal_diagram};
pub use crate::render::lane::DrawLane;
pub use crate::render::map::{DrawMap, RenderOrder};
pub use crate::render::pedestrian::DrawPedestrian;
pub use crate::render::turn::{DrawCrosswalk, DrawTurn};
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Distance, Pt2D};
use map_model::Map;
use sim::{DrawCarInput, VehicleType};
use std::f64;

const PARCEL_BOUNDARY_THICKNESS: Distance = Distance::const_meters(0.5);
const EXTRA_SHAPE_THICKNESS: Distance = Distance::const_meters(1.0);
const EXTRA_SHAPE_POINT_RADIUS: Distance = Distance::const_meters(1.0);

const BIG_ARROW_THICKNESS: Distance = Distance::const_meters(0.5);

const TURN_ICON_ARROW_THICKNESS: Distance = Distance::const_meters(0.15);
const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(2.0);
pub const CROSSWALK_LINE_THICKNESS: Distance = Distance::const_meters(0.25);

pub const MIN_ZOOM_FOR_MARKINGS: f64 = 5.0;

// Does something belong here or as a method on ID? If it ONLY applies to renderable things, then
// here. For example, trips aren't drawn, so it's meaningless to ask what their bounding box is.
pub trait Renderable {
    fn get_id(&self) -> ID;
    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx);
    fn get_bounds(&self) -> Bounds;
    fn contains_pt(&self, pt: Pt2D) -> bool;
    // Higher z-ordered objects are drawn later
    fn get_zorder(&self) -> isize {
        0
    }
}

pub struct RenderOptions {
    // The "main" color for the object, if available.
    pub color: Option<Color>,
    // TODO This should be accessible through ctx...
    pub debug_mode: bool,
    pub is_selected: bool,
    pub show_all_detail: bool,
}

pub fn draw_vehicle(input: DrawCarInput, map: &Map) -> Box<Renderable> {
    if input.vehicle_type == VehicleType::Bike {
        Box::new(DrawBike::new(input))
    } else {
        Box::new(DrawCar::new(input, map))
    }
}

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
mod traffic_signal;
mod turn;

use crate::helpers::{ColorScheme, ID};
pub use crate::render::area::DrawArea;
use crate::render::bike::DrawBike;
use crate::render::car::DrawCar;
pub use crate::render::extra_shape::ExtraShapeID;
pub use crate::render::intersection::{calculate_corners, DrawIntersection};
pub use crate::render::lane::DrawLane;
pub use crate::render::map::{AgentCache, DrawMap};
pub use crate::render::pedestrian::DrawPedestrian;
pub use crate::render::road::DrawRoad;
pub use crate::render::traffic_signal::{draw_signal_cycle, TrafficSignalDiagram};
pub use crate::render::turn::DrawTurn;
use ezgui::{Color, GfxCtx, Prerender};
use geom::{Distance, Polygon, Pt2D};
use map_model::{IntersectionID, Map};
use sim::{DrawCarInput, Sim, VehicleType};
use std::collections::HashMap;

pub const MIN_ZOOM_FOR_DETAIL: f64 = 1.0;

const EXTRA_SHAPE_THICKNESS: Distance = Distance::const_meters(1.0);
const EXTRA_SHAPE_POINT_RADIUS: Distance = Distance::const_meters(1.0);

const BIG_ARROW_THICKNESS: Distance = Distance::const_meters(0.5);

const TURN_ICON_ARROW_THICKNESS: Distance = Distance::const_meters(0.15);
const TURN_ICON_ARROW_LENGTH: Distance = Distance::const_meters(2.0);
pub const CROSSWALK_LINE_THICKNESS: Distance = Distance::const_meters(0.25);

pub const OUTLINE_THICKNESS: Distance = Distance::const_meters(0.5);

// Does something belong here or as a method on ID? If it ONLY applies to renderable things, then
// here. For example, trips aren't drawn, so it's meaningless to ask what their bounding box is.
pub trait Renderable {
    fn get_id(&self) -> ID;
    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, ctx: &DrawCtx);
    // Higher z-ordered objects are drawn later. Default to low so roads at -1 don't vanish.
    fn get_zorder(&self) -> isize {
        -5
    }
    // This outline is drawn over the base object to show that it's selected. It also represents
    // the boundaries for quadtrees. This isn't called often; don't worry about caching.
    fn get_outline(&self, map: &Map) -> Polygon;
    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        self.get_outline(map).contains_pt(pt)
    }
}

pub fn draw_vehicle(
    input: DrawCarInput,
    map: &Map,
    prerender: &Prerender,
    cs: &ColorScheme,
) -> Box<Renderable> {
    if input.id.1 == VehicleType::Bike {
        Box::new(DrawBike::new(input, map, prerender, cs))
    } else {
        Box::new(DrawCar::new(input, map, prerender, cs))
    }
}

pub struct DrawCtx<'a> {
    pub cs: &'a ColorScheme,
    pub map: &'a Map,
    pub draw_map: &'a DrawMap,
    pub sim: &'a Sim,
}

pub struct DrawOptions {
    pub override_colors: HashMap<ID, Color>,
    pub suppress_traffic_signal_details: Option<IntersectionID>,
    pub geom_debug_mode: bool,
    pub suppress_unzoomed_agents: bool,
    pub label_buildings: bool,
}

impl DrawOptions {
    pub fn new() -> DrawOptions {
        DrawOptions {
            override_colors: HashMap::new(),
            suppress_traffic_signal_details: None,
            geom_debug_mode: false,
            suppress_unzoomed_agents: false,
            label_buildings: false,
        }
    }

    pub fn color(&self, id: ID) -> Option<Color> {
        self.override_colors.get(&id).cloned()
    }
}

use geom::{Distance, Polygon, Pt2D};
use map_model::{IntersectionID, Map, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};
use sim::{DrawCarInput, VehicleType};
use widgetry::{GfxCtx, Prerender};

use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
pub use crate::render::area::DrawArea;
use crate::render::bike::DrawBike;
pub use crate::render::building::DrawBuilding;
use crate::render::car::DrawCar;
pub use crate::render::intersection::{calculate_corners, DrawIntersection};
pub use crate::render::map::{AgentCache, DrawMap, UnzoomedAgents};
pub use crate::render::pedestrian::{DrawPedCrowd, DrawPedestrian};
pub use crate::render::turn::{DrawMovement, DrawUberTurnGroup};

mod area;
mod bike;
mod building;
mod bus_stop;
mod car;
mod intersection;
mod lane;
mod map;
mod parking_lot;
mod pedestrian;
mod road;
pub mod traffic_signal;
mod turn;

pub const BIG_ARROW_THICKNESS: Distance = Distance::const_meters(0.5);

pub const CROSSWALK_LINE_THICKNESS: Distance = Distance::const_meters(0.15);

pub const OUTLINE_THICKNESS: Distance = Distance::const_meters(0.5);

// Does something belong here or as a method on ID? If it ONLY applies to renderable things, then
// here. For example, trips aren't drawn, so it's meaningless to ask what their bounding box is.
pub trait Renderable {
    // TODO This is expensive for the PedCrowd case. :( Returning a borrow is awkward, because most
    // Renderables are better off storing the inner ID directly.
    fn get_id(&self) -> ID;
    // Only traffic signals need UI. :\
    fn draw(&self, g: &mut GfxCtx, app: &App, opts: &DrawOptions);
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

fn draw_vehicle(
    input: DrawCarInput,
    map: &Map,
    prerender: &Prerender,
    cs: &ColorScheme,
) -> Box<dyn Renderable> {
    if input.id.1 == VehicleType::Bike {
        Box::new(DrawBike::new(input, map, prerender, cs))
    } else {
        Box::new(DrawCar::new(input, map, prerender, cs))
    }
}

// TODO Borrow, don't clone, and fix up lots of places storing indirect things to populate
// DrawOptions.
#[derive(Clone)]
pub struct DrawOptions {
    pub suppress_traffic_signal_details: Vec<IntersectionID>,
    pub label_buildings: bool,
}

impl DrawOptions {
    pub fn new() -> DrawOptions {
        DrawOptions {
            suppress_traffic_signal_details: Vec::new(),
            label_buildings: false,
        }
    }
}

pub fn unzoomed_agent_radius(vt: Option<VehicleType>) -> Distance {
    // Lane thickness is a little hard to see, so double it. Most of the time, the circles don't
    // leak out of the road too much.
    if vt.is_some() {
        4.0 * NORMAL_LANE_THICKNESS
    } else {
        4.0 * SIDEWALK_THICKNESS
    }
}

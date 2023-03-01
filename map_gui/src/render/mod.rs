//! Render static map elements.

use geom::{Bounds, Distance, Pt2D, Tessellation};
use map_model::{IntersectionID, Map};
use widgetry::GfxCtx;

pub use crate::render::area::DrawArea;
pub use crate::render::building::DrawBuilding;
pub use crate::render::intersection::{calculate_corners, DrawIntersection};
pub use crate::render::map::DrawMap;
pub use crate::render::turn::DrawMovement;
use crate::{AppLike, ID};

mod area;
mod building;
mod intersection;
mod lane;
mod map;
mod parking_lot;
mod road;
pub mod traffic_signal;
mod transit_stop;
mod turn;

pub const BIG_ARROW_THICKNESS: Distance = Distance::const_meters(0.5);

pub const OUTLINE_THICKNESS: Distance = Distance::const_meters(0.5);

// Does something belong here or as a method on ID? If it ONLY applies to renderable things, then
// here. For example, trips aren't drawn, so it's meaningless to ask what their bounding box is.
pub trait Renderable {
    fn get_id(&self) -> ID;
    // Only traffic signals need UI. :\
    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, opts: &DrawOptions);
    // Higher z-ordered objects are drawn later. Default to low so roads at -1 don't vanish.
    fn get_zorder(&self) -> isize {
        -5
    }
    // This outline is drawn over the base object to show that it's selected. It also represents
    // the boundaries for quadtrees. This isn't called often; don't worry about caching.
    fn get_outline(&self, map: &Map) -> Tessellation;
    fn get_bounds(&self, map: &Map) -> Bounds;
    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool;
}

/// Control how the map is drawn.
pub struct DrawOptions {
    /// Don't draw the current traffic signal state.
    pub suppress_traffic_signal_details: Vec<IntersectionID>,
    /// Label every building.
    pub label_buildings: bool,
}

impl DrawOptions {
    /// Default options for drawing a map.
    pub fn new() -> DrawOptions {
        DrawOptions {
            suppress_traffic_signal_details: Vec::new(),
            label_buildings: false,
        }
    }
}

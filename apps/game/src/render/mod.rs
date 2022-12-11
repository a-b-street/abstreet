use geom::{Distance, Pt2D, Tessellation};
use map_gui::colors::ColorScheme;
use map_gui::render::DrawOptions;
use map_gui::AppLike;
use map_model::{Map, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};
use sim::{DrawCarInput, PersonID, Sim, VehicleType};
use widgetry::{Color, GfxCtx, Prerender};

pub use crate::render::agents::{AgentCache, UnzoomedAgents};
use crate::render::bike::DrawBike;
use crate::render::car::DrawCar;
pub use crate::render::pedestrian::{DrawPedCrowd, DrawPedestrian};
use crate::ID;

mod agents;
mod bike;
mod car;
mod pedestrian;

// Like map_gui's Renderable, but uses our ID type
pub trait GameRenderable {
    // TODO This is expensive for the PedCrowd case. :( Returning a borrow is awkward, because most
    // Renderables are better off storing the inner ID directly.
    fn get_id(&self) -> ID;
    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, opts: &DrawOptions);
    // Higher z-ordered objects are drawn later. Default to low so roads at -1 don't vanish.
    fn get_zorder(&self) -> isize {
        -5
    }
    // This outline is drawn over the base object to show that it's selected. It also represents
    // the boundaries for quadtrees. This isn't called often; don't worry about caching.
    fn get_outline(&self, map: &Map) -> Tessellation;
    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool;
}

impl<R: map_gui::render::Renderable> GameRenderable for R {
    fn get_id(&self) -> ID {
        self.get_id().into()
    }
    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, opts: &DrawOptions) {
        self.draw(g, app, opts);
    }
    fn get_zorder(&self) -> isize {
        self.get_zorder()
    }
    fn get_outline(&self, map: &Map) -> Tessellation {
        self.get_outline(map)
    }
    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        self.contains_pt(pt, map)
    }
}

fn draw_vehicle(
    input: DrawCarInput,
    map: &Map,
    sim: &Sim,
    prerender: &Prerender,
    cs: &ColorScheme,
) -> Box<dyn GameRenderable> {
    if input.id.vehicle_type == VehicleType::Bike {
        Box::new(DrawBike::new(input, map, sim, prerender, cs))
    } else {
        Box::new(DrawCar::new(input, map, sim, prerender, cs))
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

/// If the sim has highlighted people, then fade all others out.
fn grey_out_unhighlighted_people(color: Color, person: &Option<PersonID>, sim: &Sim) -> Color {
    if let Some(ref highlighted) = sim.get_highlighted_people() {
        if person
            .as_ref()
            .map(|p| !highlighted.contains(p))
            .unwrap_or(false)
        {
            return color.tint(0.5);
        }
    }
    color
}

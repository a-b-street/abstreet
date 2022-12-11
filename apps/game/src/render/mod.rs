use geom::Distance;
use map_gui::colors::ColorScheme;
use map_gui::render::Renderable;
use map_model::{Map, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};
use sim::{DrawCarInput, PersonID, Sim, VehicleType};
use widgetry::{Color, Prerender};

pub use crate::render::agents::{AgentCache, UnzoomedAgents};
use crate::render::bike::DrawBike;
use crate::render::car::DrawCar;
pub use crate::render::pedestrian::{DrawPedCrowd, DrawPedestrian};

mod agents;
mod bike;
mod car;
mod pedestrian;

fn draw_vehicle(
    input: DrawCarInput,
    map: &Map,
    sim: &Sim,
    prerender: &Prerender,
    cs: &ColorScheme,
) -> Box<dyn Renderable> {
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

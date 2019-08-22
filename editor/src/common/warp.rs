use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::ui::{PerMapUI, UI};
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Warper, Wizard};
use geom::Pt2D;
use map_model::{raw_data, AreaID, BuildingID, IntersectionID, LaneID, RoadID};
use sim::{PedestrianID, TripID};
use std::usize;

const WARP_TO_CAM_ZOOM: f64 = 10.0;

pub struct EnteringWarp;
impl EnteringWarp {
    pub fn new() -> Box<dyn State> {
        WizardState::new(Box::new(warp_to))
    }
}

fn warp_to(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);
    let to = wizard.input_string("Warp to what?")?;
    if let Some((id, pt)) = warp_point(&to, &ui.primary) {
        return Some(Transition::ReplaceWithMode(
            Box::new(Warping {
                warper: Warper::new(ctx, pt, Some(WARP_TO_CAM_ZOOM)),
                id: Some(id),
            }),
            EventLoopMode::Animation,
        ));
    }
    if wizard.acknowledge("Bad warp ID", vec![&format!("{} isn't a valid ID", to)]) {
        return Some(Transition::Pop);
    }
    None
}

pub struct Warping {
    pub warper: Warper,
    pub id: Option<ID>,
}

impl State for Warping {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some(evmode) = self.warper.event(ctx) {
            Transition::KeepWithMode(evmode)
        } else {
            ui.primary.current_selection = self.id.clone();
            Transition::Pop
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &UI) {}
}

fn warp_point(line: &str, primary: &PerMapUI) -> Option<(ID, Pt2D)> {
    if line.is_empty() {
        return None;
    }

    let id = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let id = RoadID(idx);
                if let Some(r) = primary.map.maybe_get_r(id) {
                    ID::Lane(r.children_forwards[0].0)
                } else {
                    return None;
                }
            }
            'l' => ID::Lane(LaneID(idx)),
            'i' => ID::Intersection(IntersectionID(idx)),
            'b' => ID::Building(BuildingID(idx)),
            'a' => ID::Area(AreaID(idx)),
            'p' => ID::Pedestrian(PedestrianID(idx)),
            'c' => {
                // This one gets more complicated. :)
                if let Some(id) = primary.sim.lookup_car_id(idx) {
                    ID::Car(id)
                } else {
                    return None;
                }
            }
            't' => ID::Trip(TripID(idx)),
            'T' => {
                if let Some(id) = primary.map.lookup_turn_by_idx(idx) {
                    ID::Turn(id)
                } else {
                    return None;
                }
            }
            'I' => {
                let stable_id = raw_data::StableIntersectionID(idx);
                if let Some(i) = primary
                    .map
                    .all_intersections()
                    .iter()
                    .find(|i| i.stable_id == stable_id)
                {
                    ID::Intersection(i.id)
                } else {
                    return None;
                }
            }
            'R' => {
                let stable_id = raw_data::StableRoadID(idx);
                if let Some(r) = primary
                    .map
                    .all_roads()
                    .iter()
                    .find(|r| r.stable_id == stable_id)
                {
                    ID::Lane(r.children_forwards[0].0)
                } else {
                    return None;
                }
            }
            _ => {
                return None;
            }
        },
        Err(_) => {
            return None;
        }
    };
    if let Some(pt) = id.canonical_point(primary) {
        println!("Warping to {:?}", id);
        Some((id, pt))
    } else {
        None
    }
}

use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::ui::{PerMapUI, UI};
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Warper, Wizard};
use geom::Pt2D;
use map_model::{AreaID, BuildingID, IntersectionID, LaneID, RoadID};
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
    if let Some((id, pt, cam_zoom)) = warp_point(&to, &ui.primary) {
        return Some(Transition::ReplaceWithMode(
            Warping::new(ctx, pt, Some(cam_zoom), id, &mut ui.primary),
            EventLoopMode::Animation,
        ));
    }
    if wizard.acknowledge("Bad warp ID", vec![&format!("{} isn't a valid ID", to)]) {
        return Some(Transition::Pop);
    }
    None
}

pub struct Warping {
    warper: Warper,
    id: Option<ID>,
}

impl Warping {
    pub fn new(
        ctx: &EventCtx,
        pt: Pt2D,
        target_cam_zoom: Option<f64>,
        id: Option<ID>,
        primary: &mut PerMapUI,
    ) -> Box<dyn State> {
        primary.last_warped_from = Some((ctx.canvas.center_to_map_pt(), ctx.canvas.cam_zoom));
        Box::new(Warping {
            warper: Warper::new(ctx, pt, target_cam_zoom),
            id,
        })
    }
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

fn warp_point(line: &str, primary: &PerMapUI) -> Option<(Option<ID>, Pt2D, f64)> {
    if line.is_empty() {
        return None;
    }
    // TODO Weird magic shortcut to go to last spot. What should this be?
    if line == "j" {
        if let Some((pt, zoom)) = primary.last_warped_from {
            return Some((None, pt, zoom));
        }
        return None;
    }

    let id = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let r = primary.map.maybe_get_r(RoadID(idx))?;
                ID::Lane(r.children_forwards[0].0)
            }
            'l' => ID::Lane(LaneID(idx)),
            'i' => ID::Intersection(IntersectionID(idx)),
            'b' => ID::Building(BuildingID(idx)),
            'a' => ID::Area(AreaID(idx)),
            'p' => ID::Pedestrian(PedestrianID(idx)),
            'c' => {
                // This one gets more complicated. :)
                let c = primary.sim.lookup_car_id(idx)?;
                ID::Car(c)
            }
            't' => {
                let a = primary.sim.trip_to_agent(TripID(idx)).ok()?;
                ID::from_agent(a)
            }
            'T' => {
                let t = primary.map.lookup_turn_by_idx(idx)?;
                ID::Turn(t)
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
        Some((Some(id), pt, WARP_TO_CAM_ZOOM))
    } else {
        None
    }
}

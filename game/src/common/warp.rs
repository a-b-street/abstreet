use crate::app::{App, PerMap};
use crate::common::Tab;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::sandbox::SandboxMode;
use ezgui::{EventCtx, GfxCtx, Warper, Wizard};
use geom::Pt2D;
use map_model::{AreaID, BuildingID, IntersectionID, LaneID, RoadID};
use maplit::btreemap;
use sim::{PedestrianID, PersonID, TripID};
use std::collections::BTreeMap;

const WARP_TO_CAM_ZOOM: f64 = 10.0;

pub struct EnteringWarp;
impl EnteringWarp {
    pub fn new() -> Box<dyn State> {
        WizardState::new(Box::new(warp_to))
    }
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
        primary: &mut PerMap,
    ) -> Box<dyn State> {
        primary.last_warped_from = Some((ctx.canvas.center_to_map_pt(), ctx.canvas.cam_zoom));
        Box::new(Warping {
            warper: Warper::new(ctx, pt, target_cam_zoom),
            id,
        })
    }
}

impl State for Warping {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        if let Some(evmode) = self.warper.event(ctx) {
            Transition::KeepWithMode(evmode)
        } else {
            if let Some(id) = self.id.clone() {
                Transition::PopWithData(Box::new(move |state, app, ctx| {
                    // Other states pretty much don't use info panels.
                    if let Some(ref mut s) = state.downcast_mut::<SandboxMode>() {
                        let mut actions = s.contextual_actions();
                        s.controls.common.as_mut().unwrap().launch_info_panel(
                            ctx,
                            app,
                            Tab::from_id(app, id),
                            &mut actions,
                        );
                    }
                }))
            } else {
                Transition::Pop
            }
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

fn warp_to(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);
    let to = wizard.input_string("Warp to what?")?;
    if let Some(t) = inner_warp(ctx, app, &to) {
        Some(t)
    } else {
        Some(Transition::Replace(msg(
            "Bad warp ID",
            vec![format!("{} isn't a valid ID", to)],
        )))
    }
}

fn inner_warp(ctx: &mut EventCtx, app: &mut App, line: &str) -> Option<Transition> {
    if line.is_empty() {
        return None;
    }
    // TODO Weird magic shortcut to go to last spot. What should this be?
    if line == "j" {
        if let Some((pt, zoom)) = app.primary.last_warped_from {
            return Some(Transition::Replace(Warping::new(
                ctx,
                pt,
                Some(zoom),
                None,
                &mut app.primary,
            )));
        }
        return None;
    }

    let id = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let r = app.primary.map.maybe_get_r(RoadID(idx))?;
                ID::Lane(r.children_forwards[0].0)
            }
            'l' => ID::Lane(LaneID(idx)),
            'i' => ID::Intersection(IntersectionID(idx)),
            'b' => ID::Building(BuildingID(idx)),
            'a' => ID::Area(AreaID(idx)),
            'p' => ID::Pedestrian(PedestrianID(idx)),
            'P' => {
                let id = PersonID(idx);
                app.primary.sim.lookup_person(id)?;
                return Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
                    // Other states pretty much don't use info panels.
                    if let Some(ref mut s) = state.downcast_mut::<SandboxMode>() {
                        let mut actions = s.contextual_actions();
                        s.controls.common.as_mut().unwrap().launch_info_panel(
                            ctx,
                            app,
                            Tab::PersonTrips(id, BTreeMap::new()),
                            &mut actions,
                        );
                    }
                })));
            }
            'c' => {
                // This one gets more complicated. :)
                let c = app.primary.sim.lookup_car_id(idx)?;
                ID::Car(c)
            }
            't' => {
                let trip = TripID(idx);
                let person = app.primary.sim.trip_to_person(trip);
                return Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
                    // Other states pretty much don't use info panels.
                    if let Some(ref mut s) = state.downcast_mut::<SandboxMode>() {
                        let mut actions = s.contextual_actions();
                        s.controls.common.as_mut().unwrap().launch_info_panel(
                            ctx,
                            app,
                            Tab::PersonTrips(person, btreemap! {trip => true}),
                            &mut actions,
                        );
                    }
                })));
            }
            'T' => {
                let t = app.primary.map.lookup_turn_by_idx(idx)?;
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
    if let Some(pt) = id.canonical_point(&app.primary) {
        println!("Warping to {:?}", id);
        Some(Transition::Replace(Warping::new(
            ctx,
            pt,
            Some(WARP_TO_CAM_ZOOM),
            Some(id),
            &mut app.primary,
        )))
    } else {
        None
    }
}

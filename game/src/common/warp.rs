use std::collections::BTreeMap;

use geom::Pt2D;
use map_gui::tools::{grey_out_map, PopupMsg};
use map_gui::ID;
use map_model::{AreaID, BuildingID, BusRouteID, IntersectionID, LaneID, ParkingLotID, RoadID};
use sim::{PedestrianID, PersonID, TripID};
use widgetry::{
    Btn, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt, Warper, Widget,
};

use crate::app::{App, PerMap, Transition};
use crate::info::{OpenTrip, Tab};
use crate::sandbox::SandboxMode;

const WARP_TO_CAM_ZOOM: f64 = 10.0;

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
    ) -> Box<dyn State<App>> {
        primary.last_warped_from = Some((ctx.canvas.center_to_map_pt(), ctx.canvas.cam_zoom));
        Box::new(Warping {
            warper: Warper::new(ctx, pt, target_cam_zoom),
            id,
        })
    }
}

impl State<App> for Warping {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        if self.warper.event(ctx) {
            Transition::Keep
        } else {
            if let Some(id) = self.id.clone() {
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
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
                    })),
                ])
            } else {
                Transition::Pop
            }
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

pub struct DebugWarp {
    panel: Panel,
}

impl DebugWarp {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let c = ctx.style().hotkey_color;
        Box::new(DebugWarp {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Warp to an object by ID").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                "Example: r42 is Road #42".draw_text(ctx),
                // T
                // his
                //
                // i
                // s
                //
                // d
                // isorienting...
                Text::from_all(vec![
                    Line("r").fg(c),
                    Line("oad, "),
                    Line("l").fg(c),
                    Line("ane, "),
                    Line("i").fg(c),
                    Line("ntersection, "),
                    Line("b").fg(c),
                    Line("uilding, "),
                    Line("p").fg(c),
                    Line("edestrian, "),
                    Line("c").fg(c),
                    Line("ar, "),
                    Line("t").fg(c),
                    Line("rip, "),
                    Line("P").fg(c),
                    Line("erson, "),
                    Line("R").fg(c),
                    Line("oute, parking "),
                    Line("L").fg(c),
                    Line("ot"),
                ])
                .draw(ctx),
                Text::from_all(vec![
                    Line("Or "),
                    Line("j").fg(c),
                    Line("ump to the previous position"),
                ])
                .draw(ctx),
                Widget::text_entry(ctx, String::new(), true).named("input"),
                Btn::text_fg("Go!").build_def(ctx, Key::Enter),
            ]))
            .build(ctx),
        })
    }
}

impl State<App> for DebugWarp {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Go!" => {
                    let input = self.panel.text_box("input");
                    if let Some(t) = warp_to_id(ctx, app, &input) {
                        t
                    } else {
                        Transition::Replace(PopupMsg::new(
                            ctx,
                            "Bad warp ID",
                            vec![format!("{} isn't a valid ID", input)],
                        ))
                    }
                }
                _ => unreachable!(),
            },
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

fn warp_to_id(ctx: &mut EventCtx, app: &mut App, line: &str) -> Option<Transition> {
    if line.is_empty() {
        return None;
    }
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
                ID::Lane(r.lanes_ltr()[0].0)
            }
            'R' => {
                let r = BusRouteID(idx);
                app.primary.map.maybe_get_br(r)?;
                return Some(Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        // Other states pretty much don't use info panels.
                        if let Some(ref mut s) = state.downcast_mut::<SandboxMode>() {
                            let mut actions = s.contextual_actions();
                            s.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::BusRoute(r),
                                &mut actions,
                            );
                        }
                    })),
                ]));
            }
            'l' => ID::Lane(LaneID(idx)),
            'L' => ID::ParkingLot(ParkingLotID(idx)),
            'i' => ID::Intersection(IntersectionID(idx)),
            'b' => ID::Building(BuildingID(idx)),
            'a' => ID::Area(AreaID(idx)),
            'p' => ID::Pedestrian(PedestrianID(idx)),
            'P' => {
                let id = PersonID(idx);
                app.primary.sim.lookup_person(id)?;
                return Some(Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
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
                    })),
                ]));
            }
            'c' => {
                // This one gets more complicated. :)
                let c = app.primary.sim.lookup_car_id(idx)?;
                ID::Car(c)
            }
            't' => {
                let trip = TripID(idx);
                let person = app.primary.sim.trip_to_person(trip)?;
                return Some(Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        // Other states pretty much don't use info panels.
                        if let Some(ref mut s) = state.downcast_mut::<SandboxMode>() {
                            let mut actions = s.contextual_actions();
                            s.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::PersonTrips(person, OpenTrip::single(trip)),
                                &mut actions,
                            );
                        }
                    })),
                ]));
            }
            _ => {
                return None;
            }
        },
        Err(_) => {
            return None;
        }
    };
    if let Some(pt) = app.primary.canonical_point(id.clone()) {
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

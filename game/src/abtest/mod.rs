pub mod setup;

use crate::app::{App, PerMap};
use crate::common::{tool_panel, CommonState, ContextualActions};
use crate::debug::DebugMode;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::MIN_ZOOM_FOR_DETAIL;
use abstutil::Timer;
use ezgui::{lctrl, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, Text};
use geom::{Circle, Distance, Line, PolyLine};
use map_model::{Map, NORMAL_LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use sim::{Sim, SimOptions, TripID};

// TODO Controls have been removed.
pub struct ABTestMode {
    diff_trip: Option<DiffOneTrip>,
    diff_all: Option<DiffAllTrips>,
    common: CommonState,
    tool_panel: WrappedComposite,
    test_name: String,
    flipped: bool,
}

impl ABTestMode {
    pub fn new(ctx: &mut EventCtx, app: &mut App, test_name: &str) -> ABTestMode {
        app.primary.current_selection = None;

        ABTestMode {
            diff_trip: None,
            diff_all: None,
            common: CommonState::new(),
            tool_panel: tool_panel(ctx, app),
            test_name: test_name.to_string(),
            flipped: false,
        }
    }
}

impl State for ABTestMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        {
            let mut txt = Text::new();
            if self.flipped {
                txt.add(Line("B").fg(Color::CYAN));
            } else {
                txt.add(Line("A").fg(Color::RED));
            }
            txt.append(Line(format!(
                " - {}",
                app.primary.map.get_edits().edits_name
            )));
            if let Some(ref diff) = self.diff_trip {
                txt.add(Line(format!("Showing diff for {}", diff.trip)));
            } else if let Some(ref diff) = self.diff_all {
                txt.add(Line(format!(
                    "Showing diffs for all. {} trips same, {} differ",
                    diff.same_trips,
                    diff.lines.len()
                )));
            }
            // TODO Stick this info somewhere
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx, app)));
        }

        if false {
            // swap
            let secondary = app.secondary.take().unwrap();
            let primary = std::mem::replace(&mut app.primary, secondary);
            app.secondary = Some(primary);
            self.recalculate_stuff(app, ctx);

            self.flipped = !self.flipped;
        }

        if false {
            // save state
            ctx.loading_screen("savestate", |_, timer| {
                timer.start("save all state");
                self.savestate(app);
                timer.stop("save all state");
            });
        }

        if self.diff_trip.is_some() {
            if false {
                // stop diffing trips
                self.diff_trip = None;
            }
        } else if self.diff_all.is_some() {
            if false {
                // stop diffing trips
                self.diff_all = None;
            }
        } else {
            if app.primary.current_selection.is_none() && false {
                // diff all trips
                self.diff_all = Some(DiffAllTrips::new(
                    &mut app.primary,
                    app.secondary.as_mut().unwrap(),
                ));
            } else if let Some(agent) = app
                .primary
                .current_selection
                .as_ref()
                .and_then(|id| id.agent_id())
            {
                if let Some(trip) = app.primary.sim.agent_to_trip(agent) {
                    // TODO Contextual action, Key::B, show parallel world
                    if false {
                        self.diff_trip = Some(DiffOneTrip::new(
                            trip,
                            &app.primary,
                            app.secondary.as_ref().unwrap(),
                        ));
                    }
                }
            }
        }

        /*if let Some(dt) = self.speed.event(ctx, app.primary.sim.time()) {
            app.primary.sim.step(&app.primary.map, dt);
            {
                let s = app.secondary.as_mut().unwrap();
                s.sim.step(&s.map, dt);
            }
            self.recalculate_stuff(app, ctx);
        }*/

        if let Some(t) = self.common.event(ctx, app, &mut Actions {}) {
            return t;
        }
        match self.tool_panel.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => t,
            // TODO Confirm first
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "back" => Transition::Pop,
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.common.draw(g, app);
        self.tool_panel.draw(g);

        if let Some(ref diff) = self.diff_trip {
            diff.draw(g, app);
        }
        if let Some(ref diff) = self.diff_all {
            diff.draw(g, app);
        }
    }

    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut App) {
        //self.speed.pause();
    }

    fn on_destroy(&mut self, ctx: &mut EventCtx, app: &mut App) {
        ctx.loading_screen("exit A/B test mode", |_, timer| {
            timer.start("destroy secondary sim");
            // TODO Should we clear edits too?
            app.primary.clear_sim();

            app.secondary = None;
            timer.stop("destroy secondary sim");
        });
    }
}

impl ABTestMode {
    fn recalculate_stuff(&mut self, app: &mut App, ctx: &EventCtx) {
        if let Some(diff) = self.diff_trip.take() {
            self.diff_trip = Some(DiffOneTrip::new(
                diff.trip,
                &app.primary,
                app.secondary.as_ref().unwrap(),
            ));
        }
        if self.diff_all.is_some() {
            self.diff_all = Some(DiffAllTrips::new(
                &mut app.primary,
                app.secondary.as_mut().unwrap(),
            ));
        }

        app.recalculate_current_selection(ctx);
    }

    fn savestate(&mut self, app: &mut App) {
        // Preserve the original order!
        if self.flipped {
            let secondary = app.secondary.take().unwrap();
            let primary = std::mem::replace(&mut app.primary, secondary);
            app.secondary = Some(primary);
        }

        // Temporarily move everything into this structure.
        let blank_map = Map::blank();
        let mut secondary = app.secondary.take().unwrap();
        let ss = ABTestSavestate {
            primary_map: std::mem::replace(&mut app.primary.map, Map::blank()),
            primary_sim: std::mem::replace(
                &mut app.primary.sim,
                Sim::new(&blank_map, SimOptions::new("tmp"), &mut Timer::throwaway()),
            ),
            secondary_map: std::mem::replace(&mut secondary.map, Map::blank()),
            secondary_sim: std::mem::replace(
                &mut secondary.sim,
                Sim::new(&blank_map, SimOptions::new("tmp"), &mut Timer::throwaway()),
            ),
        };

        abstutil::write_binary(
            abstutil::path_ab_test_save(
                ss.primary_map.get_name(),
                &self.test_name,
                ss.primary_sim.time().as_filename(),
            ),
            &ss,
        );

        // Restore everything.
        app.primary.sim = ss.primary_sim;
        app.primary.map = ss.primary_map;
        app.secondary = Some(PerMap {
            map: ss.secondary_map,
            draw_map: secondary.draw_map,
            sim: ss.secondary_sim,
            current_selection: secondary.current_selection,
            current_flags: secondary.current_flags,
            last_warped_from: None,
        });

        if self.flipped {
            let secondary = app.secondary.take().unwrap();
            let primary = std::mem::replace(&mut app.primary, secondary);
            app.secondary = Some(primary);
        }
    }
}

pub struct DiffOneTrip {
    trip: TripID,
    // These are all optional because mode-changes might cause temporary interruptions.
    // Just point from primary world agent to secondary world agent.
    line: Option<Line>,
    primary_route: Option<PolyLine>,
    secondary_route: Option<PolyLine>,
}

impl DiffOneTrip {
    fn new(trip: TripID, primary: &PerMap, secondary: &PerMap) -> DiffOneTrip {
        let pt1 = primary
            .sim
            .get_canonical_pt_per_trip(trip, &primary.map)
            .ok();
        let pt2 = secondary
            .sim
            .get_canonical_pt_per_trip(trip, &secondary.map)
            .ok();
        let line = if let (Some(pt1), Some(pt2)) = (pt1, pt2) {
            Line::maybe_new(pt1, pt2)
        } else {
            None
        };
        let primary_agent = primary.sim.trip_to_agent(trip).ok();
        let secondary_agent = secondary.sim.trip_to_agent(trip).ok();
        if primary_agent.is_none() || secondary_agent.is_none() {
            println!("{} isn't present in both sims", trip);
        }
        DiffOneTrip {
            trip,
            line,
            primary_route: primary_agent
                .and_then(|a| primary.sim.trace_route(a, &primary.map, None)),
            secondary_route: secondary_agent
                .and_then(|a| secondary.sim.trace_route(a, &secondary.map, None)),
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if let Some(l) = &self.line {
            g.draw_line(Color::YELLOW.alpha(0.5), NORMAL_LANE_THICKNESS, l);
        }
        if let Some(t) = &self.primary_route {
            g.draw_polygon(
                app.cs.after_changes.alpha(0.5),
                &t.make_polygons(NORMAL_LANE_THICKNESS),
            );
        }
        if let Some(t) = &self.secondary_route {
            g.draw_polygon(
                app.cs.before_changes.alpha(0.5),
                &t.make_polygons(NORMAL_LANE_THICKNESS),
            );
        }
    }
}

pub struct DiffAllTrips {
    same_trips: usize,
    // TODO Or do we want to augment DrawCars and DrawPeds, so we get automatic quadtree support?
    lines: Vec<Line>,
}

impl DiffAllTrips {
    fn new(primary: &mut PerMap, secondary: &mut PerMap) -> DiffAllTrips {
        let trip_positions1 = primary.sim.get_trip_positions(&primary.map);
        let trip_positions2 = secondary.sim.get_trip_positions(&secondary.map);
        let mut same_trips = 0;
        let mut lines: Vec<Line> = Vec::new();
        for (trip, pt1) in &trip_positions1.canonical_pt_per_trip {
            if let Some(pt2) = trip_positions2.canonical_pt_per_trip.get(trip) {
                if let Some(l) = Line::maybe_new(*pt1, *pt2) {
                    lines.push(l);
                } else {
                    same_trips += 1;
                }
            }
        }
        DiffAllTrips { same_trips, lines }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        let mut batch = GeomBatch::new();
        let color = Color::YELLOW.alpha(0.5);
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            // TODO Refactor with UI
            let radius = Distance::meters(10.0) / g.canvas.cam_zoom;
            for line in &self.lines {
                batch.push(color, Circle::new(line.pt1(), radius).to_polygon());
            }
        } else {
            for line in &self.lines {
                batch.push(color, line.make_polygons(NORMAL_LANE_THICKNESS));
            }
        }
        batch.draw(g);
    }
}

#[derive(Serialize, Deserialize)]
pub struct ABTestSavestate {
    primary_map: Map,
    primary_sim: Sim,
    secondary_map: Map,
    secondary_sim: Sim,
}

struct Actions;
impl ContextualActions for Actions {
    fn actions(&self, _: &App, _: ID) -> Vec<(Key, String)> {
        unreachable!()
    }
    fn execute(
        &mut self,
        _: &mut EventCtx,
        _: &mut App,
        _: ID,
        _: String,
        _: &mut bool,
    ) -> Transition {
        unreachable!()
    }
    fn is_paused(&self) -> bool {
        unreachable!()
    }
}

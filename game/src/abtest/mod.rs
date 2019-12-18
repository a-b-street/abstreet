pub mod setup;

use crate::common::{tool_panel, AgentTools, CommonState};
use crate::debug::DebugMode;
use crate::game::{State, Transition};
use crate::managed::{Composite, Outcome};
use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::{PerMapUI, UI};
use abstutil::Timer;
use ezgui::{hotkey, lctrl, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, ModalMenu, Text};
use geom::{Circle, Distance, Line, PolyLine};
use map_model::{Map, LANE_THICKNESS};
use serde_derive::{Deserialize, Serialize};
use sim::{Sim, SimOptions, TripID, TripMode};

// TODO I took out speed controls
pub struct ABTestMode {
    menu: ModalMenu,
    primary_agent_tools: AgentTools,
    secondary_agent_tools: AgentTools,
    diff_trip: Option<DiffOneTrip>,
    diff_all: Option<DiffAllTrips>,
    common: CommonState,
    tool_panel: Composite,
    test_name: String,
    flipped: bool,
}

impl ABTestMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI, test_name: &str) -> ABTestMode {
        ui.primary.current_selection = None;

        ABTestMode {
            menu: ModalMenu::new(
                "A/B Test Mode",
                vec![
                    (hotkey(Key::S), "swap"),
                    (hotkey(Key::D), "diff all trips"),
                    (hotkey(Key::A), "stop diffing trips"),
                    (hotkey(Key::O), "save state"),
                    // TODO load arbitrary savestate
                ],
                ctx,
            ),
            primary_agent_tools: AgentTools::new(),
            secondary_agent_tools: AgentTools::new(),
            diff_trip: None,
            diff_all: None,
            common: CommonState::new(),
            tool_panel: tool_panel(ctx, Vec::new()),
            test_name: test_name.to_string(),
            flipped: false,
        }
    }
}

impl State for ABTestMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        {
            let mut txt = Text::new();
            if self.flipped {
                txt.add(Line("B").fg(Color::CYAN));
            } else {
                txt.add(Line("A").fg(Color::RED));
            }
            txt.append(Line(format!(
                " - {}",
                ui.primary.map.get_edits().edits_name
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
            let (active, unfinished, by_mode) = ui.primary.sim.num_trips();
            txt.add(Line(format!("Active trips: {}", active)));
            txt.add(Line(format!("Unfinished trips: {}", unfinished)));
            txt.add(Line(format!(
                "Peds {}, Bikes {}, Cars {}, Buses {}",
                by_mode[&TripMode::Walk],
                by_mode[&TripMode::Bike],
                by_mode[&TripMode::Drive],
                by_mode[&TripMode::Transit]
            )));
            self.menu.set_info(ctx, txt);
        }
        self.menu.event(ctx);

        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }

        if ui.opts.dev && ctx.input.new_was_pressed(lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx)));
        }

        if self.menu.action("swap") {
            let secondary = ui.secondary.take().unwrap();
            let primary = std::mem::replace(&mut ui.primary, secondary);
            ui.secondary = Some(primary);
            self.recalculate_stuff(ui, ctx);

            std::mem::swap(
                &mut self.primary_agent_tools,
                &mut self.secondary_agent_tools,
            );
            self.flipped = !self.flipped;
        }

        if let Some(t) = self.primary_agent_tools.event(ctx, ui, &mut self.menu) {
            return t;
        }

        if self.menu.action("save state") {
            ctx.loading_screen("savestate", |_, timer| {
                timer.start("save all state");
                self.savestate(ui);
                timer.stop("save all state");
            });
        }

        if self.diff_trip.is_some() {
            if self.menu.action("stop diffing trips") {
                self.diff_trip = None;
            }
        } else if self.diff_all.is_some() {
            if self.menu.action("stop diffing trips") {
                self.diff_all = None;
            }
        } else {
            if ui.primary.current_selection.is_none() && self.menu.action("diff all trips") {
                self.diff_all = Some(DiffAllTrips::new(
                    &mut ui.primary,
                    ui.secondary.as_mut().unwrap(),
                ));
            } else if let Some(agent) = ui
                .primary
                .current_selection
                .as_ref()
                .and_then(|id| id.agent_id())
            {
                if let Some(trip) = ui.primary.sim.agent_to_trip(agent) {
                    if ui
                        .per_obj
                        .action(ctx, Key::B, format!("Show {}'s parallel world", agent))
                    {
                        self.diff_trip = Some(DiffOneTrip::new(
                            trip,
                            &ui.primary,
                            ui.secondary.as_ref().unwrap(),
                        ));
                    }
                }
            }
        }

        /*if let Some(dt) = self.speed.event(ctx, ui.primary.sim.time()) {
            ui.primary.sim.step(&ui.primary.map, dt);
            {
                let s = ui.secondary.as_mut().unwrap();
                s.sim.step(&s.map, dt);
            }
            self.recalculate_stuff(ui, ctx);
        }*/

        if let Some(t) = self.common.event(ctx, ui) {
            return t;
        }
        match self.tool_panel.event(ctx, ui) {
            Some(Outcome::Transition(t)) => t,
            // TODO Confirm first
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "back" => Transition::Pop,
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.common.draw(g, ui);
        self.tool_panel.draw(g);

        if let Some(ref diff) = self.diff_trip {
            diff.draw(g, ui);
        }
        if let Some(ref diff) = self.diff_all {
            diff.draw(g, ui);
        }
        self.menu.draw(g);
        self.primary_agent_tools.draw(g);
    }

    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut UI) {
        //self.speed.pause();
    }

    fn on_destroy(&mut self, ctx: &mut EventCtx, ui: &mut UI) {
        ctx.loading_screen("exit A/B test mode", |_, timer| {
            timer.start("destroy secondary sim");
            // TODO Should we clear edits too?
            ui.primary.clear_sim();

            ui.secondary = None;
            timer.stop("destroy secondary sim");
        });
    }
}

impl ABTestMode {
    fn recalculate_stuff(&mut self, ui: &mut UI, ctx: &EventCtx) {
        if let Some(diff) = self.diff_trip.take() {
            self.diff_trip = Some(DiffOneTrip::new(
                diff.trip,
                &ui.primary,
                ui.secondary.as_ref().unwrap(),
            ));
        }
        if self.diff_all.is_some() {
            self.diff_all = Some(DiffAllTrips::new(
                &mut ui.primary,
                ui.secondary.as_mut().unwrap(),
            ));
        }

        ui.recalculate_current_selection(ctx);
    }

    fn savestate(&mut self, ui: &mut UI) {
        // Preserve the original order!
        if self.flipped {
            let secondary = ui.secondary.take().unwrap();
            let primary = std::mem::replace(&mut ui.primary, secondary);
            ui.secondary = Some(primary);
        }

        // Temporarily move everything into this structure.
        let blank_map = Map::blank();
        let mut secondary = ui.secondary.take().unwrap();
        let ss = ABTestSavestate {
            primary_map: std::mem::replace(&mut ui.primary.map, Map::blank()),
            primary_sim: std::mem::replace(
                &mut ui.primary.sim,
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
        ui.primary.sim = ss.primary_sim;
        ui.primary.map = ss.primary_map;
        ui.secondary = Some(PerMapUI {
            map: ss.secondary_map,
            draw_map: secondary.draw_map,
            sim: ss.secondary_sim,
            current_selection: secondary.current_selection,
            current_flags: secondary.current_flags,
            last_warped_from: None,
        });

        if self.flipped {
            let secondary = ui.secondary.take().unwrap();
            let primary = std::mem::replace(&mut ui.primary, secondary);
            ui.secondary = Some(primary);
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
    fn new(trip: TripID, primary: &PerMapUI, secondary: &PerMapUI) -> DiffOneTrip {
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

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(l) = &self.line {
            g.draw_line(
                ui.cs.get_def("diff agents line", Color::YELLOW.alpha(0.5)),
                LANE_THICKNESS,
                l,
            );
        }
        if let Some(t) = &self.primary_route {
            g.draw_polygon(
                ui.cs.get_def("primary agent route", Color::RED.alpha(0.5)),
                &t.make_polygons(LANE_THICKNESS),
            );
        }
        if let Some(t) = &self.secondary_route {
            g.draw_polygon(
                ui.cs
                    .get_def("secondary agent route", Color::BLUE.alpha(0.5)),
                &t.make_polygons(LANE_THICKNESS),
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
    fn new(primary: &mut PerMapUI, secondary: &mut PerMapUI) -> DiffAllTrips {
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

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut batch = GeomBatch::new();
        let color = ui.cs.get("diff agents line");
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            // TODO Refactor with UI
            let radius = Distance::meters(10.0) / g.canvas.cam_zoom;
            for line in &self.lines {
                batch.push(color, Circle::new(line.pt1(), radius).to_polygon());
            }
        } else {
            for line in &self.lines {
                batch.push(color, line.make_polygons(LANE_THICKNESS));
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

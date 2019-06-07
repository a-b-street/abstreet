mod setup;

use crate::common::{CommonState, SpeedControls};
use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::ui::{PerMapUI, ShowEverything, UI};
use ezgui::{hotkey, Color, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard};
use geom::{Duration, Line, PolyLine};
use map_model::LANE_THICKNESS;
use sim::TripID;

pub struct ABTestMode {
    menu: ModalMenu,
    speed: SpeedControls,
    pub state: State,
    // TODO Urgh, hack. Need to be able to take() it to switch states sometimes.
    pub secondary: Option<PerMapUI>,
    diff_trip: Option<DiffOneTrip>,
    diff_all: Option<DiffAllTrips>,
    // TODO Not present in Setup state.
    common: CommonState,
}

pub enum State {
    Setup(setup::ABTestSetup),
    Playing,
}

impl ABTestMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI) -> ABTestMode {
        ui.primary.current_selection = None;

        ABTestMode {
            menu: ModalMenu::new(
                "A/B Test Mode",
                vec![
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (hotkey(Key::LeftBracket), "slow down"),
                        (hotkey(Key::RightBracket), "speed up"),
                        (hotkey(Key::Space), "pause/resume"),
                        (hotkey(Key::M), "step forwards 0.1s"),
                        (hotkey(Key::S), "swap"),
                        (hotkey(Key::D), "diff all trips"),
                        (hotkey(Key::B), "stop diffing trips"),
                    ],
                    CommonState::modal_menu_entries(),
                ]
                .concat(),
                ctx,
            ),
            speed: SpeedControls::new(ctx, None),
            state: State::Setup(setup::ABTestSetup::Pick(Wizard::new())),
            secondary: None,
            diff_trip: None,
            diff_all: None,
            common: CommonState::new(),
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::ABTest(ref mut mode) => {
                match mode.state {
                    State::Setup(_) => {
                        setup::ABTestSetup::event(state, ctx);
                        EventLoopMode::InputOnly
                    }
                    State::Playing => {
                        let mut txt = Text::prompt("A/B Test Mode");
                        txt.add_line(state.ui.primary.map.get_edits().edits_name.clone());
                        if let Some(ref diff) = mode.diff_trip {
                            txt.add_line(format!("Showing diff for {}", diff.trip));
                        } else if let Some(ref diff) = mode.diff_all {
                            txt.add_line(format!(
                                "Showing diffs for all. {} equivalent trips",
                                diff.same_trips
                            ));
                        }
                        txt.add_line(state.ui.primary.sim.summary());
                        txt.add_line(mode.speed.modal_status_line());
                        mode.menu.handle_event(ctx, Some(txt));

                        ctx.canvas.handle_event(ctx.input);
                        if ctx.redo_mouseover() {
                            state.ui.primary.current_selection =
                                state.ui.recalculate_current_selection(
                                    ctx,
                                    &state.ui.primary.sim,
                                    &ShowEverything::new(),
                                    false,
                                );
                        }
                        if let Some(evmode) = mode.common.event(ctx, &mut state.ui, &mut mode.menu)
                        {
                            return evmode;
                        }

                        if mode.menu.action("quit") {
                            // TODO This shouldn't be necessary when we plumb state around instead of
                            // sharing it in the old structure.
                            state.ui.primary.reset_sim();
                            // Note destroying mode.secondary has some noticeable delay.
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                            return EventLoopMode::InputOnly;
                        }

                        if mode.menu.action("swap") {
                            let secondary = mode.secondary.take().unwrap();
                            let primary = std::mem::replace(&mut state.ui.primary, secondary);
                            mode.secondary = Some(primary);
                            mode.recalculate_stuff(&mut state.ui, ctx);
                        }

                        if mode.diff_trip.is_some() {
                            if mode.menu.action("stop diffing trips") {
                                mode.diff_trip = None;
                            }
                        } else if mode.diff_all.is_some() {
                            if mode.menu.action("stop diffing trips") {
                                mode.diff_all = None;
                            }
                        } else {
                            if state.ui.primary.current_selection.is_none()
                                && mode.menu.action("diff all trips")
                            {
                                mode.diff_all = Some(DiffAllTrips::new(
                                    &mut state.ui.primary,
                                    mode.secondary.as_mut().unwrap(),
                                ));
                            } else if let Some(agent) = state
                                .ui
                                .primary
                                .current_selection
                                .and_then(|id| id.agent_id())
                            {
                                if let Some(trip) = state.ui.primary.sim.agent_to_trip(agent) {
                                    if ctx.input.contextual_action(
                                        Key::B,
                                        &format!("Show {}'s parallel world", agent),
                                    ) {
                                        mode.diff_trip = Some(DiffOneTrip::new(
                                            trip,
                                            &state.ui.primary,
                                            mode.secondary.as_ref().unwrap(),
                                        ));
                                    }
                                }
                            }
                        }

                        if let Some(dt) =
                            mode.speed
                                .event(ctx, &mut mode.menu, state.ui.primary.sim.time())
                        {
                            mode.step(dt, &mut state.ui, ctx);
                        }

                        if mode.speed.is_paused() {
                            if mode.menu.action("step forwards 0.1s") {
                                mode.step(Duration::seconds(0.1), &mut state.ui, ctx);
                            }
                            EventLoopMode::InputOnly
                        } else {
                            EventLoopMode::Animation
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    fn step(&mut self, dt: Duration, ui: &mut UI, ctx: &EventCtx) {
        ui.primary.sim.step(&ui.primary.map, dt);
        {
            let s = self.secondary.as_mut().unwrap();
            s.sim.step(&s.map, dt);
        }
        self.recalculate_stuff(ui, ctx);
    }

    fn recalculate_stuff(&mut self, ui: &mut UI, ctx: &EventCtx) {
        if let Some(diff) = self.diff_trip.take() {
            self.diff_trip = Some(DiffOneTrip::new(
                diff.trip,
                &ui.primary,
                self.secondary.as_ref().unwrap(),
            ));
        }
        if self.diff_all.is_some() {
            self.diff_all = Some(DiffAllTrips::new(
                &mut ui.primary,
                self.secondary.as_mut().unwrap(),
            ));
        }

        ui.primary.current_selection =
            ui.recalculate_current_selection(ctx, &ui.primary.sim, &ShowEverything::new(), false);
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        match state.mode {
            Mode::ABTest(ref mode) => match mode.state {
                State::Setup(ref setup) => {
                    state.ui.draw(
                        g,
                        DrawOptions::new(),
                        &state.ui.primary.sim,
                        &ShowEverything::new(),
                    );
                    setup.draw(g);
                }
                _ => {
                    state.ui.draw(
                        g,
                        mode.common.draw_options(&state.ui),
                        &state.ui.primary.sim,
                        &ShowEverything::new(),
                    );
                    mode.common.draw(g, &state.ui);

                    if let Some(ref diff) = mode.diff_trip {
                        diff.draw(g, &state.ui);
                    }
                    if let Some(ref diff) = mode.diff_all {
                        diff.draw(g, &state.ui);
                    }
                    mode.menu.draw(g);
                    mode.speed.draw(g);
                }
            },
            _ => unreachable!(),
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
        let pt1 = primary.sim.get_canonical_pt_per_trip(trip, &primary.map);
        let pt2 = secondary
            .sim
            .get_canonical_pt_per_trip(trip, &secondary.map);
        let line = if pt1.is_some() && pt2.is_some() {
            Line::maybe_new(pt1.unwrap(), pt2.unwrap())
        } else {
            None
        };
        let primary_agent = primary.sim.trip_to_agent(trip);
        let secondary_agent = secondary.sim.trip_to_agent(trip);
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
        let stats1 = primary.sim.get_stats(&primary.map);
        let stats2 = secondary.sim.get_stats(&secondary.map);
        let mut same_trips = 0;
        let mut lines: Vec<Line> = Vec::new();
        for (trip, pt1) in &stats1.canonical_pt_per_trip {
            if let Some(pt2) = stats2.canonical_pt_per_trip.get(trip) {
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
        for line in &self.lines {
            g.draw_line(ui.cs.get("diff agents line"), LANE_THICKNESS, line);
        }
    }
}

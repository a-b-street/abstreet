mod score;
mod spawner;
mod thruput_stats;
mod time_travel;
mod trip_stats;

use crate::common::{
    time_controls, AgentTools, CommonState, ObjectColorer, ObjectColorerBuilder, RoadColorer,
    RoadColorerBuilder, RouteExplorer, SpeedControls, TripExplorer,
};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::ui::{PerMapUI, ShowEverything, UI};
use abstutil::Counter;
use ezgui::{
    hotkey, lctrl, Choice, Color, EventCtx, EventLoopMode, GfxCtx, Key, Line, ModalMenu, Text,
    Wizard,
};
use geom::Duration;
use sim::{ParkingSpot, Sim};
use std::collections::HashSet;

pub struct SandboxMode {
    speed: SpeedControls,
    agent_tools: AgentTools,
    pub time_travel: time_travel::InactiveTimeTravel,
    trip_stats: trip_stats::TripStats,
    thruput_stats: thruput_stats::ThruputStats,
    common: CommonState,
    parking_heatmap: Option<(Duration, RoadColorer)>,
    intersection_delay_heatmap: Option<(Duration, ObjectColorer)>,
    menu: ModalMenu,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> SandboxMode {
        SandboxMode {
            speed: SpeedControls::new(ctx, None),
            agent_tools: AgentTools::new(ctx),
            time_travel: time_travel::InactiveTimeTravel::new(),
            trip_stats: trip_stats::TripStats::new(
                ui.primary.current_flags.sim_flags.opts.record_stats,
            ),
            thruput_stats: thruput_stats::ThruputStats::new(),
            common: CommonState::new(),
            parking_heatmap: None,
            intersection_delay_heatmap: None,
            menu: ModalMenu::new(
                "Sandbox Mode",
                vec![
                    vec![
                        (hotkey(Key::RightBracket), "speed up"),
                        (hotkey(Key::LeftBracket), "slow down"),
                        (hotkey(Key::Space), "pause/resume"),
                        (hotkey(Key::M), "step forwards 0.1s"),
                        (hotkey(Key::N), "step forwards 10 mins"),
                        (hotkey(Key::B), "jump to specific time"),
                    ],
                    vec![
                        (hotkey(Key::O), "save sim state"),
                        (hotkey(Key::Y), "load previous sim state"),
                        (hotkey(Key::U), "load next sim state"),
                        (None, "pick a savestate to load"),
                        (hotkey(Key::X), "reset sim"),
                        (hotkey(Key::S), "start a scenario"),
                    ],
                    vec![
                        // TODO Strange to always have this. Really it's a case of stacked modal?
                        (hotkey(Key::A), "show/hide parking availability"),
                        (hotkey(Key::I), "show/hide intersection delay"),
                        (hotkey(Key::T), "start time traveling"),
                        (hotkey(Key::Q), "scoreboard"),
                        (None, "trip stats"),
                        (None, "throughput stats"),
                    ],
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (lctrl(Key::D), "debug mode"),
                        (lctrl(Key::E), "edit mode"),
                        (hotkey(Key::J), "warp"),
                        (hotkey(Key::K), "navigate"),
                        (hotkey(Key::SingleQuote), "shortcuts"),
                        (hotkey(Key::F1), "take a screenshot"),
                    ],
                ],
                ctx,
            ),
        }
    }
}

impl State for SandboxMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.time_travel.record(ui);
        self.trip_stats.record(ui);
        self.thruput_stats.record(ui);

        {
            let mut txt = Text::prompt("Sandbox Mode");
            txt.add(Line(ui.primary.sim.summary()));
            self.menu.handle_event(ctx, Some(txt));
        }

        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        if let Some(t) = self.common.event(ctx, ui, &mut self.menu) {
            return t;
        }

        if let Some(new_state) = spawner::AgentSpawner::new(ctx, ui, &mut self.menu) {
            return Transition::Push(new_state);
        }
        if let Some(explorer) = RouteExplorer::new(ctx, ui) {
            return Transition::Push(Box::new(explorer));
        }
        if let Some(explorer) = TripExplorer::new(ctx, ui) {
            return Transition::Push(Box::new(explorer));
        }

        if let Some(t) = self.agent_tools.event(ctx, ui) {
            return t;
        }
        if ui.primary.current_selection.is_none() && self.menu.action("start time traveling") {
            return self.time_travel.start(ctx, ui);
        }
        if self.menu.action("scoreboard") {
            return Transition::Push(Box::new(score::Scoreboard::new(ctx, ui)));
        }
        if self.menu.action("trip stats") {
            if let Some(s) = trip_stats::ShowStats::new(&self.trip_stats, ui, ctx) {
                return Transition::Push(Box::new(s));
            } else {
                println!("No trip stats available");
            }
        }
        if self.menu.action("throughput stats") {
            return Transition::Push(Box::new(thruput_stats::ShowStats::new(
                &self.thruput_stats,
                ui,
                ctx,
            )));
        }
        if self.menu.action("show/hide parking availability") {
            if self.parking_heatmap.is_some() {
                self.parking_heatmap = None;
            } else {
                self.parking_heatmap = Some((
                    ui.primary.sim.time(),
                    calculate_parking_heatmap(ctx, &ui.primary),
                ));
            }
        }
        if self
            .parking_heatmap
            .as_ref()
            .map(|(t, _)| *t != ui.primary.sim.time())
            .unwrap_or(false)
        {
            self.parking_heatmap = Some((
                ui.primary.sim.time(),
                calculate_parking_heatmap(ctx, &ui.primary),
            ));
        }
        if self.menu.action("show/hide intersection delay") {
            if self.intersection_delay_heatmap.is_some() {
                self.intersection_delay_heatmap = None;
            } else {
                self.intersection_delay_heatmap = Some((
                    ui.primary.sim.time(),
                    calculate_intersection_delay(ctx, &ui.primary),
                ));
            }
        }
        if self
            .intersection_delay_heatmap
            .as_ref()
            .map(|(t, _)| *t != ui.primary.sim.time())
            .unwrap_or(false)
        {
            self.intersection_delay_heatmap = Some((
                ui.primary.sim.time(),
                calculate_intersection_delay(ctx, &ui.primary),
            ));
        }

        if self.menu.action("quit") {
            return Transition::Pop;
        }
        if self.menu.action("debug mode") {
            return Transition::Push(Box::new(DebugMode::new(ctx, ui)));
        }
        if self.menu.action("edit mode") {
            return Transition::Replace(Box::new(EditMode::new(ctx, ui)));
        }
        if let Some(ID::Building(b)) = ui.primary.current_selection {
            let cars = ui
                .primary
                .sim
                .get_offstreet_parked_cars(b)
                .into_iter()
                .map(|p| p.vehicle.id)
                .collect::<Vec<_>>();
            if !cars.is_empty()
                && ctx
                    .input
                    .contextual_action(Key::P, format!("examine {} cars parked here", cars.len()))
            {
                return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, _| {
                    let _id = wiz.wrap(ctx).choose("Examine which car?", || {
                        cars.iter()
                            .map(|c| Choice::new(c.to_string(), *c))
                            .collect()
                    })?;
                    Some(Transition::Pop)
                })));
            }
        }

        if let Some(dt) = self.speed.event(ctx, &mut self.menu, ui.primary.sim.time()) {
            // If speed is too high, don't be unresponsive for too long.
            // TODO This should probably match the ezgui framerate.
            ui.primary
                .sim
                .time_limited_step(&ui.primary.map, dt, Duration::seconds(0.1));
            ui.recalculate_current_selection(ctx);
        }

        if self.speed.is_paused() {
            if !ui.primary.sim.is_empty() && self.menu.action("reset sim") {
                ui.primary.reset_sim();
                return Transition::Replace(Box::new(SandboxMode::new(ctx, ui)));
            }
            if self.menu.action("save sim state") {
                ctx.loading_screen("savestate", |_, timer| {
                    timer.start("save sim state");
                    ui.primary.sim.save();
                    timer.stop("save sim state");
                });
            }
            if self.menu.action("load previous sim state") {
                ctx.loading_screen("load previous savestate", |ctx, mut timer| {
                    let prev_state = ui
                        .primary
                        .sim
                        .find_previous_savestate(ui.primary.sim.time());
                    match prev_state
                        .clone()
                        .and_then(|path| Sim::load_savestate(path, &mut timer).ok())
                    {
                        Some(new_sim) => {
                            ui.primary.sim = new_sim;
                            ui.recalculate_current_selection(ctx);
                        }
                        None => println!("Couldn't load previous savestate {:?}", prev_state),
                    }
                });
            }
            if self.menu.action("load next sim state") {
                ctx.loading_screen("load next savestate", |ctx, mut timer| {
                    let next_state = ui.primary.sim.find_next_savestate(ui.primary.sim.time());
                    match next_state
                        .clone()
                        .and_then(|path| Sim::load_savestate(path, &mut timer).ok())
                    {
                        Some(new_sim) => {
                            ui.primary.sim = new_sim;
                            ui.recalculate_current_selection(ctx);
                        }
                        None => println!("Couldn't load next savestate {:?}", next_state),
                    }
                });
            }
            if self.menu.action("pick a savestate to load") {
                return Transition::Push(WizardState::new(Box::new(load_savestate)));
            }

            if let Some(t) = time_controls(ctx, ui, &mut self.menu) {
                return t;
            }

            Transition::Keep
        } else {
            Transition::KeepWithMode(EventLoopMode::Animation)
        }
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // TODO Oh no, these're actually exclusive, represent that better.
        if let Some((_, ref c)) = self.parking_heatmap {
            c.draw(g, ui);
        } else if let Some((_, ref c)) = self.intersection_delay_heatmap {
            c.draw(g, ui);
        } else {
            ui.draw(
                g,
                self.common.draw_options(ui),
                &ui.primary.sim,
                &ShowEverything::new(),
            );
        }
        self.common.draw(g, ui);
        self.agent_tools.draw(g, ui);
        self.menu.draw(g);
        self.speed.draw(g);
    }

    fn on_suspend(&mut self, _: &mut UI) {
        self.speed.pause();
    }
}

fn load_savestate(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let path = ui.primary.sim.save_dir();

    let ss = wiz.wrap(ctx).choose_string("Load which savestate?", || {
        abstutil::list_dir(std::path::Path::new(&path))
    })?;

    ctx.loading_screen("load savestate", |ctx, mut timer| {
        ui.primary.sim = Sim::load_savestate(ss, &mut timer).expect("Can't load savestate");
        ui.recalculate_current_selection(ctx);
    });
    Some(Transition::Pop)
}

fn calculate_parking_heatmap(ctx: &mut EventCtx, primary: &PerMapUI) -> RoadColorer {
    let awful = Color::BLACK;
    let bad = Color::RED;
    let meh = Color::YELLOW;
    let good = Color::GREEN;
    let mut colorer = RoadColorerBuilder::new(
        "parking availability",
        vec![
            ("< 10%", awful),
            ("< 30%", bad),
            ("< 60%", meh),
            (">= 60%", good),
        ],
    );

    let lane = |spot| match spot {
        ParkingSpot::Onstreet(l, _) => l,
        ParkingSpot::Offstreet(b, _) => primary
            .map
            .get_b(b)
            .parking
            .as_ref()
            .unwrap()
            .driving_pos
            .lane(),
    };

    let mut filled = Counter::new();
    let mut avail = Counter::new();
    let mut keys = HashSet::new();
    let (filled_spots, avail_spots) = primary.sim.get_all_parking_spots();
    for spot in filled_spots {
        let l = lane(spot);
        keys.insert(l);
        filled.inc(l);
    }
    for spot in avail_spots {
        let l = lane(spot);
        keys.insert(l);
        avail.inc(l);
    }

    for l in keys {
        let open = avail.get(l);
        let closed = filled.get(l);
        let percent = (open as f64) / ((open + closed) as f64);
        let color = if percent >= 0.6 {
            good
        } else if percent > 0.3 {
            meh
        } else if percent > 0.1 {
            bad
        } else {
            awful
        };
        colorer.add(l, color, &primary.map);
    }

    colorer.build(ctx, &primary.map)
}

fn calculate_intersection_delay(ctx: &mut EventCtx, primary: &PerMapUI) -> ObjectColorer {
    let fast = Color::GREEN;
    let meh = Color::YELLOW;
    let slow = Color::RED;
    let mut colorer = ObjectColorerBuilder::new(
        "intersection delay (90%ile)",
        vec![("< 10s", fast), ("<= 60s", meh), ("> 60s", slow)],
    );

    for i in primary.map.all_intersections() {
        let delays = primary.sim.get_intersection_delays(i.id);
        if let Some(d) = delays.percentile(90.0) {
            let color = if d < Duration::seconds(10.0) {
                fast
            } else if d <= Duration::seconds(60.0) {
                meh
            } else {
                slow
            };
            colorer.add(ID::Intersection(i.id), color);
        }
    }

    colorer.build(ctx, &primary.map)
}

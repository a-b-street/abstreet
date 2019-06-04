mod route_explorer;
mod route_viewer;
mod show_activity;
mod spawner;
mod time_travel;

use crate::common::CommonState;
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::ui::ShowEverything;
use abstutil::elapsed_seconds;
use ezgui::{hotkey, lctrl, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Slider, Text, Wizard};
use geom::Duration;
use sim::{Benchmark, Sim, TripID};
use std::time::Instant;

const ADJUST_SPEED: f64 = 0.1;
// TODO hardcoded cap for now...
const SPEED_CAP: f64 = 60.0;

pub struct SandboxMode {
    speed_slider: Slider,
    following: Option<TripID>,
    route_viewer: route_viewer::RouteViewer,
    show_activity: show_activity::ShowActivity,
    time_travel: time_travel::TimeTravel,
    state: State,
    // TODO Not while Spawning or TimeTraveling or ExploringRoute...
    common: CommonState,
    menu: ModalMenu,
}

enum State {
    Paused,
    Running {
        last_step: Instant,
        benchmark: Benchmark,
        speed: String,
    },
    Spawning(spawner::AgentSpawner),
    TimeTraveling,
    ExploringRoute(route_explorer::RouteExplorer),
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx) -> SandboxMode {
        let mut speed_slider = Slider::new(None);
        speed_slider.set_percent(ctx, 1.0 / SPEED_CAP);
        SandboxMode {
            speed_slider,
            state: State::Paused,
            following: None,
            route_viewer: route_viewer::RouteViewer::Inactive,
            show_activity: show_activity::ShowActivity::Inactive,
            time_travel: time_travel::TimeTravel::new(),
            common: CommonState::new(),
            menu: ModalMenu::new(
                "Sandbox Mode",
                vec![
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (hotkey(Key::LeftBracket), "slow down sim"),
                        (hotkey(Key::RightBracket), "speed up sim"),
                        (hotkey(Key::O), "save sim state"),
                        (hotkey(Key::Y), "load previous sim state"),
                        (hotkey(Key::U), "load next sim state"),
                        (hotkey(Key::Space), "run/pause sim"),
                        (hotkey(Key::M), "step forwards 0.1s"),
                        (hotkey(Key::N), "step forwards 10 mins"),
                        (hotkey(Key::X), "reset sim"),
                        (hotkey(Key::S), "seed the sim with agents"),
                        // TODO Strange to always have this. Really it's a case of stacked modal?
                        (hotkey(Key::F), "stop following agent"),
                        (hotkey(Key::R), "stop showing agent's route"),
                        // TODO This should probably be a debug thing instead
                        (hotkey(Key::L), "show/hide route for all agents"),
                        (hotkey(Key::A), "show/hide active traffic"),
                        (hotkey(Key::T), "start time traveling"),
                        (lctrl(Key::D), "debug mode"),
                        (lctrl(Key::E), "edit mode"),
                    ],
                    CommonState::modal_menu_entries(),
                ]
                .concat(),
                ctx,
            ),
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Sandbox(ref mut mode) => {
                if let State::Spawning(ref mut spawner) = mode.state {
                    if spawner.event(ctx, &mut state.ui) {
                        mode.state = State::Paused;
                    }
                    return EventLoopMode::InputOnly;
                }
                if let State::TimeTraveling = mode.state {
                    if mode.time_travel.event(ctx) {
                        mode.state = State::Paused;
                    }
                    return EventLoopMode::InputOnly;
                }
                mode.time_travel.record(&state.ui);
                if let State::ExploringRoute(ref mut explorer) = mode.state {
                    if let Some(mode) = explorer.event(ctx, &mut state.ui) {
                        return mode;
                    }
                    mode.state = State::Paused;
                    return EventLoopMode::InputOnly;
                }

                let desired_speed = mode.speed_slider.get_percent() * SPEED_CAP;

                let mut txt = Text::prompt("Sandbox Mode");
                txt.add_line(state.ui.primary.sim.summary());
                if let State::Running { ref speed, .. } = mode.state {
                    txt.add_line(format!(
                        "Speed: {0} / desired {1:.2}x",
                        speed, desired_speed
                    ));
                } else {
                    txt.add_line(format!("Speed: paused / desired {0:.2}x", desired_speed));
                }
                if let Some(trip) = mode.following {
                    txt.add_line(format!("Following {}", trip));
                }
                match mode.route_viewer {
                    route_viewer::RouteViewer::Active(_, trip, _) => {
                        txt.add_line(format!("Showing {}'s route", trip));
                    }
                    route_viewer::RouteViewer::DebugAllRoutes(_, _) => {
                        txt.add_line("Showing all routes".to_string());
                    }
                    _ => {}
                }
                match mode.show_activity {
                    show_activity::ShowActivity::Inactive => {}
                    _ => {
                        txt.add_line("Showing active traffic".to_string());
                    }
                }
                mode.menu.handle_event(ctx, Some(txt));

                ctx.canvas.handle_event(ctx.input);
                state.ui.primary.current_selection = state.ui.handle_mouseover(
                    ctx,
                    &state.ui.primary.sim,
                    &ShowEverything::new(),
                    false,
                );
                if let Some(evmode) = mode.common.event(ctx, &mut state.ui, &mut mode.menu) {
                    return evmode;
                }

                if let Some(spawner) =
                    spawner::AgentSpawner::new(ctx, &mut state.ui, &mut mode.menu)
                {
                    mode.state = State::Spawning(spawner);
                    return EventLoopMode::InputOnly;
                }
                if let Some(explorer) = route_explorer::RouteExplorer::new(ctx, &state.ui) {
                    mode.state = State::ExploringRoute(explorer);
                    return EventLoopMode::InputOnly;
                }

                if mode.following.is_none() {
                    if let Some(agent) = state
                        .ui
                        .primary
                        .current_selection
                        .and_then(|id| id.agent_id())
                    {
                        if let Some(trip) = state.ui.primary.sim.agent_to_trip(agent) {
                            if ctx
                                .input
                                .contextual_action(Key::F, &format!("follow {}", agent))
                            {
                                mode.following = Some(trip);
                            }
                        }
                    }
                }
                if let Some(trip) = mode.following {
                    if let Some(pt) = state
                        .ui
                        .primary
                        .sim
                        .get_canonical_pt_per_trip(trip, &state.ui.primary.map)
                    {
                        ctx.canvas.center_on_map_pt(pt);
                    } else {
                        // TODO ideally they wouldnt vanish for so long according to
                        // get_canonical_point_for_trip
                        println!("{} is gone... temporarily or not?", trip);
                    }
                    if mode.menu.action("stop following agent") {
                        mode.following = None;
                    }
                }
                mode.route_viewer.event(ctx, &mut state.ui, &mut mode.menu);
                mode.show_activity.event(ctx, &mut state.ui, &mut mode.menu);
                if mode.menu.action("start time traveling") {
                    mode.state = State::TimeTraveling;
                    mode.time_travel.start(ctx, &state.ui);
                    return EventLoopMode::InputOnly;
                }

                if mode.menu.action("quit") {
                    state.mode = Mode::SplashScreen(Wizard::new(), None);
                    return EventLoopMode::InputOnly;
                }
                if mode.menu.action("debug mode") {
                    state.mode = Mode::Debug(DebugMode::new(ctx, &state.ui));
                    return EventLoopMode::InputOnly;
                }
                if mode.menu.action("edit mode") {
                    state.mode = Mode::Edit(EditMode::new(ctx, &mut state.ui));
                    return EventLoopMode::InputOnly;
                }

                if desired_speed != SPEED_CAP && mode.menu.action("speed up sim") {
                    mode.speed_slider
                        .set_percent(ctx, ((desired_speed + ADJUST_SPEED) / SPEED_CAP).min(1.0));
                } else if desired_speed != 0.0 && mode.menu.action("slow down sim") {
                    mode.speed_slider
                        .set_percent(ctx, ((desired_speed - ADJUST_SPEED) / SPEED_CAP).max(0.0));
                } else if mode.speed_slider.event(ctx) {
                    // Keep going
                }

                match mode.state {
                    State::Paused => {
                        if !state.ui.primary.sim.is_empty() && mode.menu.action("reset sim") {
                            state.ui.primary.reset_sim();
                            mode.state = State::Paused;
                            mode.following = None;
                            mode.route_viewer = route_viewer::RouteViewer::Inactive;
                            mode.show_activity = show_activity::ShowActivity::Inactive;
                        }
                        if mode.menu.action("save sim state") {
                            state.ui.primary.sim.save();
                        }
                        if mode.menu.action("load previous sim state") {
                            let prev_state = state
                                .ui
                                .primary
                                .sim
                                .find_previous_savestate(state.ui.primary.sim.time());
                            match prev_state
                                .clone()
                                .and_then(|path| Sim::load_savestate(path, None).ok())
                            {
                                Some(new_sim) => {
                                    state.ui.primary.sim = new_sim;
                                    //*ctx.recalculate_current_selection = true;
                                }
                                None => {
                                    println!("Couldn't load previous savestate {:?}", prev_state)
                                }
                            }
                        }
                        if mode.menu.action("load next sim state") {
                            let next_state = state
                                .ui
                                .primary
                                .sim
                                .find_next_savestate(state.ui.primary.sim.time());
                            match next_state
                                .clone()
                                .and_then(|path| Sim::load_savestate(path, None).ok())
                            {
                                Some(new_sim) => {
                                    state.ui.primary.sim = new_sim;
                                    //*ctx.recalculate_current_selection = true;
                                }
                                None => println!("Couldn't load next savestate {:?}", next_state),
                            }
                        }

                        if mode.menu.action("run/pause sim") {
                            mode.state = State::Running {
                                last_step: Instant::now(),
                                benchmark: state.ui.primary.sim.start_benchmark(),
                                speed: "...".to_string(),
                            };
                        } else if mode.menu.action("step forwards 0.1s") {
                            state
                                .ui
                                .primary
                                .sim
                                .step(&state.ui.primary.map, Duration::seconds(0.1));
                        //*ctx.recalculate_current_selection = true;
                        } else if mode.menu.action("step forwards 10 mins") {
                            state
                                .ui
                                .primary
                                .sim
                                .step(&state.ui.primary.map, Duration::minutes(10));
                            //*ctx.recalculate_current_selection = true;
                        }
                        EventLoopMode::InputOnly
                    }
                    State::Running {
                        ref mut last_step,
                        ref mut benchmark,
                        ref mut speed,
                    } => {
                        if mode.menu.action("run/pause sim") {
                            mode.state = State::Paused;
                        } else if ctx.input.nonblocking_is_update_event() {
                            ctx.input.use_update_event();
                            let dt = Duration::seconds(elapsed_seconds(*last_step)) * desired_speed;

                            state.ui.primary.sim.step(&state.ui.primary.map, dt);
                            //*ctx.recalculate_current_selection = true;
                            *last_step = Instant::now();

                            if benchmark.has_real_time_passed(Duration::seconds(1.0)) {
                                *speed = state.ui.primary.sim.measure_speed(benchmark, false);
                            }
                        }
                        EventLoopMode::Animation
                    }
                    State::Spawning(_) => unreachable!(),
                    State::TimeTraveling => unreachable!(),
                    State::ExploringRoute(_) => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        match state.mode {
            Mode::Sandbox(ref mode) => match mode.state {
                State::Spawning(ref spawner) => {
                    spawner.draw(g, &state.ui);
                }
                State::TimeTraveling => {
                    state.ui.draw(
                        g,
                        DrawOptions::new(),
                        &mode.time_travel,
                        &ShowEverything::new(),
                    );
                    mode.time_travel.draw(g);
                }
                State::ExploringRoute(ref explorer) => {
                    state.ui.draw(
                        g,
                        DrawOptions::new(),
                        &state.ui.primary.sim,
                        &ShowEverything::new(),
                    );
                    explorer.draw(g, &state.ui);
                }
                _ => {
                    state.ui.draw(
                        g,
                        mode.common.draw_options(&state.ui),
                        &state.ui.primary.sim,
                        &ShowEverything::new(),
                    );
                    mode.common.draw(g, &state.ui);
                    mode.route_viewer.draw(g, &state.ui);
                    mode.show_activity.draw(g, &state.ui);
                    mode.menu.draw(g);
                    mode.speed_slider.draw(g);
                }
            },
            _ => unreachable!(),
        }
    }
}

mod route_viewer;
mod show_activity;
mod spawner;
mod time_travel;

use crate::common::CommonState;
use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::ui::ShowEverything;
use abstutil::elapsed_seconds;
use ezgui::{Canvas, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard};
use geom::Duration;
use sim::{Benchmark, Sim, TripID};
use std::time::Instant;

const ADJUST_SPEED: f64 = 0.1;

pub struct SandboxMode {
    desired_speed: f64, // sim seconds per real second
    following: Option<TripID>,
    route_viewer: route_viewer::RouteViewer,
    show_activity: show_activity::ShowActivity,
    time_travel: time_travel::TimeTravel,
    state: State,
    // TODO Not while Spawning or TimeTraveling...
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
}

impl SandboxMode {
    pub fn new(canvas: &Canvas) -> SandboxMode {
        SandboxMode {
            desired_speed: 1.0,
            state: State::Paused,
            following: None,
            route_viewer: route_viewer::RouteViewer::Inactive,
            show_activity: show_activity::ShowActivity::Inactive,
            time_travel: time_travel::TimeTravel::new(canvas),
            common: CommonState::new(),
            menu: ModalMenu::hacky_new(
                "Sandbox Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::LeftBracket, "slow down sim"),
                    (Key::RightBracket, "speed up sim"),
                    (Key::O, "save sim state"),
                    (Key::Y, "load previous sim state"),
                    (Key::U, "load next sim state"),
                    (Key::Space, "run/pause sim"),
                    (Key::M, "run one step of sim"),
                    (Key::X, "reset sim"),
                    (Key::S, "seed the sim with agents"),
                    // TODO Strange to always have this. Really it's a case of stacked modal?
                    (Key::F, "stop following agent"),
                    (Key::R, "stop showing agent's route"),
                    // TODO This should probably be a debug thing instead
                    (Key::L, "show/hide route for all agents"),
                    (Key::A, "show/hide active traffic"),
                    (Key::T, "start time traveling"),
                ],
                canvas,
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
                mode.time_travel.record(&state.ui);
                if let State::TimeTraveling = mode.state {
                    if mode.time_travel.event(ctx) {
                        mode.state = State::Paused;
                    }
                    return EventLoopMode::InputOnly;
                }

                let mut txt = Text::prompt("Sandbox Mode");
                txt.add_line(state.ui.primary.sim.summary());
                if let State::Running { ref speed, .. } = mode.state {
                    txt.add_line(format!(
                        "Speed: {0} / desired {1:.2}x",
                        speed, mode.desired_speed
                    ));
                } else {
                    txt.add_line(format!(
                        "Speed: paused / desired {0:.2}x",
                        mode.desired_speed
                    ));
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
                mode.menu.update_prompt(txt, ctx);
                mode.menu.handle_event(ctx);

                ctx.canvas.handle_event(ctx.input);
                state.ui.primary.current_selection = state.ui.handle_mouseover(
                    ctx,
                    None,
                    &state.ui.primary.sim,
                    &ShowEverything::new(),
                    false,
                );
                if let Some(evmode) = mode.common.event(ctx, &state.ui) {
                    return evmode;
                }

                if let Some(spawner) =
                    spawner::AgentSpawner::new(ctx, &mut state.ui, &mut mode.menu)
                {
                    mode.state = State::Spawning(spawner);
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
                    mode.time_travel.start(state.ui.primary.sim.time());
                    // Do this again, in case recording was previously disabled.
                    mode.time_travel.record(&state.ui);
                    return EventLoopMode::InputOnly;
                }

                if mode.menu.action("quit") {
                    // TODO This shouldn't be necessary when we plumb state around instead of
                    // sharing it in the old structure.
                    state.ui.primary.sim = Sim::new(
                        &state.ui.primary.map,
                        state.ui.primary.current_flags.sim_flags.run_name.clone(),
                        None,
                    );
                    state.mode = Mode::SplashScreen(Wizard::new(), None);
                    return EventLoopMode::InputOnly;
                }

                if mode.menu.action("slow down sim") {
                    mode.desired_speed -= ADJUST_SPEED;
                    mode.desired_speed = mode.desired_speed.max(0.0);
                }
                if mode.menu.action("speed up sim") {
                    mode.desired_speed += ADJUST_SPEED;
                }
                if !state.ui.primary.sim.is_empty() && mode.menu.action("reset sim") {
                    // TODO savestate_every gets lost
                    state.ui.primary.sim = Sim::new(
                        &state.ui.primary.map,
                        state.ui.primary.current_flags.sim_flags.run_name.clone(),
                        None,
                    );
                    mode.state = State::Paused;
                }

                match mode.state {
                    State::Paused => {
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
                        } else if mode.menu.action("run one step of sim") {
                            state.ui.primary.sim.step(&state.ui.primary.map);
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
                            // TODO https://gafferongames.com/post/fix_your_timestep/
                            // TODO This doesn't interact correctly with the fixed 30 Update events sent
                            // per second. Even Benchmark is kind of wrong. I think we want to count the
                            // number of steps we've done in the last second, then stop if the speed says
                            // we should.
                            let dt_s = elapsed_seconds(*last_step);
                            if dt_s >= sim::TIMESTEP.inner_seconds() / mode.desired_speed {
                                ctx.input.use_update_event();
                                state.ui.primary.sim.step(&state.ui.primary.map);
                                //*ctx.recalculate_current_selection = true;
                                *last_step = Instant::now();

                                if benchmark.has_real_time_passed(Duration::seconds(1.0)) {
                                    *speed = state.ui.primary.sim.measure_speed(benchmark, false);
                                }
                            }
                        }
                        EventLoopMode::Animation
                    }
                    State::Spawning(_) => unreachable!(),
                    State::TimeTraveling => unreachable!(),
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
                _ => {
                    state.ui.draw(
                        g,
                        mode.common.draw_options(&state.ui),
                        &state.ui.primary.sim,
                        &ShowEverything::new(),
                    );
                    mode.common.draw(g, &state.ui);
                    mode.menu.draw(g);
                    mode.route_viewer.draw(g, &state.ui);
                    mode.show_activity.draw(g, &state.ui);
                }
            },
            _ => unreachable!(),
        }
    }
}

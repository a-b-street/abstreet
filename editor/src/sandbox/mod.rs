mod route_explorer;
mod route_viewer;
mod show_activity;
mod spawner;
mod time_travel;

use crate::common::{CommonState, SpeedControls};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::game::{GameState, Mode};
use crate::mission::input_time;
use crate::render::DrawOptions;
use crate::ui::ShowEverything;
use ezgui::{hotkey, lctrl, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard};
use geom::Duration;
use sim::{Sim, TripID};

pub struct SandboxMode {
    speed: SpeedControls,
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
    Playing,
    Spawning(spawner::AgentSpawner),
    TimeTraveling,
    ExploringRoute(route_explorer::RouteExplorer),
    JumpingToTime(Wizard),
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx) -> SandboxMode {
        SandboxMode {
            speed: SpeedControls::new(ctx, None),
            state: State::Playing,
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
                        (hotkey(Key::RightBracket), "speed up"),
                        (hotkey(Key::LeftBracket), "slow down"),
                        (hotkey(Key::Space), "pause/resume"),
                        (hotkey(Key::O), "save sim state"),
                        (hotkey(Key::Y), "load previous sim state"),
                        (hotkey(Key::U), "load next sim state"),
                        (hotkey(Key::M), "step forwards 0.1s"),
                        (hotkey(Key::N), "step forwards 10 mins"),
                        (hotkey(Key::B), "jump to specific time"),
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
            Mode::Sandbox(ref mut mode) => match mode.state {
                State::Spawning(ref mut spawner) => {
                    if spawner.event(ctx, &mut state.ui) {
                        mode.state = State::Playing;
                        mode.speed.pause();
                    }
                    EventLoopMode::InputOnly
                }
                State::TimeTraveling => {
                    if mode.time_travel.event(ctx) {
                        mode.state = State::Playing;
                        mode.speed.pause();
                    }
                    EventLoopMode::InputOnly
                }
                State::ExploringRoute(ref mut explorer) => {
                    if let Some(mode) = explorer.event(ctx, &mut state.ui) {
                        mode
                    } else {
                        mode.state = State::Playing;
                        mode.speed.pause();
                        EventLoopMode::InputOnly
                    }
                }
                State::JumpingToTime(ref mut wizard) => {
                    let mut wiz = wizard.wrap(ctx);

                    if let Some(t) = input_time(&mut wiz, "Jump to what time?") {
                        let dt = t - state.ui.primary.sim.time();
                        if dt <= Duration::ZERO {
                            if wiz.acknowledge(
                                "Bad time",
                                vec![&format!(
                                    "{} isn't after {}",
                                    t,
                                    state.ui.primary.sim.time()
                                )],
                            ) {
                                mode.state = State::Playing;
                                mode.speed.pause();
                            }
                        } else {
                            // Have to do this first for the borrow checker
                            mode.state = State::Playing;
                            mode.speed.pause();

                            if dt > Duration::ZERO {
                                ctx.loading_screen(
                                    &format!("step forwards {}", dt),
                                    |_, mut timer| {
                                        state.ui.primary.sim.timed_step(
                                            &state.ui.primary.map,
                                            dt,
                                            &mut timer,
                                        );
                                    },
                                );
                            }
                        }
                    } else if wizard.aborted() {
                        mode.state = State::Playing;
                        mode.speed.pause();
                    }
                    EventLoopMode::InputOnly
                }
                State::Playing => {
                    mode.time_travel.record(&state.ui);

                    let mut txt = Text::prompt("Sandbox Mode");
                    txt.add_line(state.ui.primary.sim.summary());
                    txt.add_line(mode.speed.modal_status_line());
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
                    if ctx.redo_mouseover() {
                        state.ui.primary.current_selection =
                            state.ui.recalculate_current_selection(
                                ctx,
                                &state.ui.primary.sim,
                                &ShowEverything::new(),
                                false,
                            );
                    }
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

                    if let Some(dt) =
                        mode.speed
                            .event(ctx, &mut mode.menu, state.ui.primary.sim.time())
                    {
                        state.ui.primary.sim.step(&state.ui.primary.map, dt);
                        state.ui.primary.current_selection =
                            state.ui.recalculate_current_selection(
                                ctx,
                                &state.ui.primary.sim,
                                &ShowEverything::new(),
                                false,
                            );
                    }

                    if mode.speed.is_paused() {
                        if !state.ui.primary.sim.is_empty() && mode.menu.action("reset sim") {
                            state.ui.primary.reset_sim();
                            mode.state = State::Playing;
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
                                .and_then(|path| Sim::load_savestate(path).ok())
                            {
                                Some(new_sim) => {
                                    state.ui.primary.sim = new_sim;
                                    state.ui.primary.current_selection =
                                        state.ui.recalculate_current_selection(
                                            ctx,
                                            &state.ui.primary.sim,
                                            &ShowEverything::new(),
                                            false,
                                        );
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
                                .and_then(|path| Sim::load_savestate(path).ok())
                            {
                                Some(new_sim) => {
                                    state.ui.primary.sim = new_sim;
                                    state.ui.primary.current_selection =
                                        state.ui.recalculate_current_selection(
                                            ctx,
                                            &state.ui.primary.sim,
                                            &ShowEverything::new(),
                                            false,
                                        );
                                }
                                None => println!("Couldn't load next savestate {:?}", next_state),
                            }
                        }

                        if mode.menu.action("step forwards 0.1s") {
                            state
                                .ui
                                .primary
                                .sim
                                .step(&state.ui.primary.map, Duration::seconds(0.1));
                            state.ui.primary.current_selection =
                                state.ui.recalculate_current_selection(
                                    ctx,
                                    &state.ui.primary.sim,
                                    &ShowEverything::new(),
                                    false,
                                );
                        } else if mode.menu.action("step forwards 10 mins") {
                            ctx.loading_screen("step forwards 10 minutes", |_, mut timer| {
                                state.ui.primary.sim.timed_step(
                                    &state.ui.primary.map,
                                    Duration::minutes(10),
                                    &mut timer,
                                );
                            });
                            state.ui.primary.current_selection =
                                state.ui.recalculate_current_selection(
                                    ctx,
                                    &state.ui.primary.sim,
                                    &ShowEverything::new(),
                                    false,
                                );
                        } else if mode.menu.action("jump to specific time") {
                            mode.state = State::JumpingToTime(Wizard::new());
                        }
                        EventLoopMode::InputOnly
                    } else {
                        EventLoopMode::Animation
                    }
                }
            },
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
                State::JumpingToTime(ref wizard) => {
                    state.ui.draw(
                        g,
                        DrawOptions::new(),
                        &state.ui.primary.sim,
                        &ShowEverything::new(),
                    );
                    wizard.draw(g);
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
                    mode.speed.draw(g);
                }
            },
            _ => unreachable!(),
        }
    }
}

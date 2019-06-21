mod route_explorer;
mod route_viewer;
mod score;
mod show_activity;
mod spawner;
mod time_travel;

use crate::common::{CommonState, SpeedControls};
//use crate::debug::DebugMode;
//use crate::edit::EditMode;
use crate::state::{State, Transition};
//use crate::mission::input_time;
use crate::render::DrawOptions;
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, lctrl, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard};
use geom::Duration;
use sim::{Sim, TripID};

pub struct SandboxMode {
    speed: SpeedControls,
    following: Option<TripID>,
    route_viewer: route_viewer::RouteViewer,
    show_activity: show_activity::ShowActivity,
    time_travel: time_travel::TimeTravel,
    // TODO Not while Spawning or TimeTraveling or ExploringRoute...
    common: CommonState,
    menu: ModalMenu,
}

/*enum State {
    TimeTraveling,
    JumpingToTime(Wizard),
}*/

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx) -> SandboxMode {
        SandboxMode {
            speed: SpeedControls::new(ctx, None),
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
                        (hotkey(Key::Q), "scoreboard"),
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
}

impl State for SandboxMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        self.time_travel.record(ui);

        let mut txt = Text::prompt("Sandbox Mode");
        txt.add_line(ui.primary.sim.summary());
        if let Some(trip) = self.following {
            txt.add_line(format!("Following {}", trip));
        }
        match self.route_viewer {
            route_viewer::RouteViewer::Active(_, trip, _) => {
                txt.add_line(format!("Showing {}'s route", trip));
            }
            route_viewer::RouteViewer::DebugAllRoutes(_, _) => {
                txt.add_line("Showing all routes".to_string());
            }
            _ => {}
        }
        match self.show_activity {
            show_activity::ShowActivity::Inactive => {}
            _ => {
                txt.add_line("Showing active traffic".to_string());
            }
        }
        self.menu.handle_event(ctx, Some(txt));

        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.primary.current_selection = ui.recalculate_current_selection(
                ctx,
                &ui.primary.sim,
                &ShowEverything::new(),
                false,
            );
        }
        if let Some(evmode) = self.common.event(ctx, ui, &mut self.menu) {
            return (Transition::Keep, evmode);
        }

        if let Some(spawner) = spawner::AgentSpawner::new(ctx, ui, &mut self.menu) {
            return (
                Transition::Push(Box::new(spawner)),
                EventLoopMode::InputOnly,
            );
        }
        if let Some(explorer) = route_explorer::RouteExplorer::new(ctx, ui) {
            return (
                Transition::Push(Box::new(explorer)),
                EventLoopMode::InputOnly,
            );
        }

        if self.following.is_none() {
            if let Some(agent) = ui.primary.current_selection.and_then(|id| id.agent_id()) {
                if let Some(trip) = ui.primary.sim.agent_to_trip(agent) {
                    if ctx
                        .input
                        .contextual_action(Key::F, &format!("follow {}", agent))
                    {
                        self.following = Some(trip);
                    }
                }
            }
        }
        if let Some(trip) = self.following {
            if let Some(pt) = ui
                .primary
                .sim
                .get_canonical_pt_per_trip(trip, &ui.primary.map)
            {
                ctx.canvas.center_on_map_pt(pt);
            } else {
                // TODO ideally they wouldnt vanish for so long according to
                // get_canonical_point_for_trip
                println!("{} is gone... temporarily or not?", trip);
            }
            if self.menu.action("stop following agent") {
                self.following = None;
            }
        }
        self.route_viewer.event(ctx, ui, &mut self.menu);
        self.show_activity.event(ctx, ui, &mut self.menu);
        if self.menu.action("start time traveling") {
            //self.state = State::TimeTraveling;
            //self.time_travel.start(ctx, ui);
            //return EventLoopMode::InputOnly;
        }
        if self.menu.action("scoreboard") {
            return (
                Transition::Push(Box::new(score::Scoreboard::new(ctx, ui))),
                EventLoopMode::InputOnly,
            );
        }

        if self.menu.action("quit") {
            return (Transition::Pop, EventLoopMode::InputOnly);
        }
        if self.menu.action("debug mode") {
            //state.mode = Mode::Debug(DebugMode::new(ctx, &state.ui));
            //return EventLoopMode::InputOnly;
        }
        if self.menu.action("edit mode") {
            //state.mode = Mode::Edit(EditMode::new(ctx, &mut state.ui));
            //return EventLoopMode::InputOnly;
        }

        if let Some(dt) = self.speed.event(ctx, &mut self.menu, ui.primary.sim.time()) {
            // If speed is too high, don't be unresponsive for too long.
            // TODO This should probably match the ezgui framerate.
            ui.primary
                .sim
                .time_limited_step(&ui.primary.map, dt, Duration::seconds(0.1));
            ui.primary.current_selection = ui.recalculate_current_selection(
                ctx,
                &ui.primary.sim,
                &ShowEverything::new(),
                false,
            );
        }

        if self.speed.is_paused() {
            if !ui.primary.sim.is_empty() && self.menu.action("reset sim") {
                ui.primary.reset_sim();
                return (
                    Transition::Replace(Box::new(SandboxMode::new(ctx))),
                    EventLoopMode::InputOnly,
                );
            }
            if self.menu.action("save sim state") {
                ui.primary.sim.save();
            }
            if self.menu.action("load previous sim state") {
                let prev_state = ui
                    .primary
                    .sim
                    .find_previous_savestate(ui.primary.sim.time());
                match prev_state
                    .clone()
                    .and_then(|path| Sim::load_savestate(path).ok())
                {
                    Some(new_sim) => {
                        ui.primary.sim = new_sim;
                        ui.primary.current_selection = ui.recalculate_current_selection(
                            ctx,
                            &ui.primary.sim,
                            &ShowEverything::new(),
                            false,
                        );
                    }
                    None => println!("Couldn't load previous savestate {:?}", prev_state),
                }
            }
            if self.menu.action("load next sim state") {
                let next_state = ui.primary.sim.find_next_savestate(ui.primary.sim.time());
                match next_state
                    .clone()
                    .and_then(|path| Sim::load_savestate(path).ok())
                {
                    Some(new_sim) => {
                        ui.primary.sim = new_sim;
                        ui.primary.current_selection = ui.recalculate_current_selection(
                            ctx,
                            &ui.primary.sim,
                            &ShowEverything::new(),
                            false,
                        );
                    }
                    None => println!("Couldn't load next savestate {:?}", next_state),
                }
            }

            if self.menu.action("step forwards 0.1s") {
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
                ui.primary.current_selection = ui.recalculate_current_selection(
                    ctx,
                    &ui.primary.sim,
                    &ShowEverything::new(),
                    false,
                );
            } else if self.menu.action("step forwards 10 mins") {
                ctx.loading_screen("step forwards 10 minutes", |_, mut timer| {
                    ui.primary
                        .sim
                        .timed_step(&ui.primary.map, Duration::minutes(10), &mut timer);
                });
                ui.primary.current_selection = ui.recalculate_current_selection(
                    ctx,
                    &ui.primary.sim,
                    &ShowEverything::new(),
                    false,
                );
            } else if self.menu.action("jump to specific time") {
                // TODO new state
                //mode.state = State::JumpingToTime(Wizard::new());
            }
            (Transition::Keep, EventLoopMode::InputOnly)
        } else {
            (Transition::Keep, EventLoopMode::Animation)
        }

        /*match state.mode {
            Mode::Sandbox(ref mut mode) => match mode.state {
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
            },
            _ => unreachable!(),
        }*/
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        ui.draw(
            g,
            self.common.draw_options(ui),
            &ui.primary.sim,
            &ShowEverything::new(),
        );
        self.common.draw(g, ui);
        self.route_viewer.draw(g, ui);
        self.show_activity.draw(g, ui);
        self.menu.draw(g);
        self.speed.draw(g);

        /*match state.mode {
            Mode::Sandbox(ref mode) => match mode.state {
                State::TimeTraveling => {
                    state.ui.draw(
                        g,
                        DrawOptions::new(),
                        &mode.time_travel,
                        &ShowEverything::new(),
                    );
                    mode.time_travel.draw(g);
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
            },
            _ => unreachable!(),
        }*/
    }
}

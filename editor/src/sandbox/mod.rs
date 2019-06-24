mod route_explorer;
mod route_viewer;
mod score;
mod show_activity;
mod spawner;
mod time_travel;

use crate::common::{CommonState, SpeedControls};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::game::{State, Transition};
use crate::mission::input_time;
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, lctrl, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard};
use geom::Duration;
use sim::{Sim, TripID};

pub struct SandboxMode {
    speed: SpeedControls,
    following: Option<TripID>,
    route_viewer: route_viewer::RouteViewer,
    show_activity: show_activity::ShowActivity,
    pub time_travel: time_travel::InactiveTimeTravel,
    common: CommonState,
    menu: ModalMenu,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx) -> SandboxMode {
        SandboxMode {
            speed: SpeedControls::new(ctx, None),
            following: None,
            route_viewer: route_viewer::RouteViewer::Inactive,
            show_activity: show_activity::ShowActivity::Inactive,
            time_travel: time_travel::InactiveTimeTravel::new(),
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
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
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
        if let Some(t) = self.common.event(ctx, ui, &mut self.menu) {
            return t;
        }

        if let Some(spawner) = spawner::AgentSpawner::new(ctx, ui, &mut self.menu) {
            return Transition::Push(Box::new(spawner));
        }
        if let Some(explorer) = route_explorer::RouteExplorer::new(ctx, ui) {
            return Transition::Push(Box::new(explorer));
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
            return self.time_travel.start(ctx, ui);
        }
        if self.menu.action("scoreboard") {
            return Transition::Push(Box::new(score::Scoreboard::new(ctx, ui)));
        }

        if self.menu.action("quit") {
            return Transition::Pop;
        }
        if self.menu.action("debug mode") {
            // TODO Replace or Push?
            return Transition::Replace(Box::new(DebugMode::new(ctx, ui)));
        }
        if self.menu.action("edit mode") {
            return Transition::Replace(Box::new(EditMode::new(ctx, ui)));
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
                return Transition::Replace(Box::new(SandboxMode::new(ctx)));
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
                return Transition::Push(Box::new(JumpingToTime {
                    wizard: Wizard::new(),
                }));
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
    }

    fn on_suspend(&mut self, _: &mut UI) {
        self.speed.pause();
    }
}

struct JumpingToTime {
    wizard: Wizard,
}

impl State for JumpingToTime {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let mut wiz = self.wizard.wrap(ctx);

        if let Some(t) = input_time(&mut wiz, "Jump to what time?") {
            let dt = t - ui.primary.sim.time();
            if dt <= Duration::ZERO {
                if wiz.acknowledge(
                    "Bad time",
                    vec![&format!("{} isn't after {}", t, ui.primary.sim.time())],
                ) {
                    return Transition::Pop;
                }
            } else {
                if dt > Duration::ZERO {
                    ctx.loading_screen(&format!("step forwards {}", dt), |_, mut timer| {
                        ui.primary.sim.timed_step(&ui.primary.map, dt, &mut timer);
                    });
                }

                return Transition::Pop;
            }
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

mod score;
mod spawner;
mod time_travel;

use crate::common::{
    time_controls, AgentTools, CommonState, RouteExplorer, SpeedControls, TripExplorer,
};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, lctrl, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, Wizard};
use geom::Duration;
use sim::Sim;

pub struct SandboxMode {
    speed: SpeedControls,
    agent_tools: AgentTools,
    pub time_travel: time_travel::InactiveTimeTravel,
    common: CommonState,
    menu: ModalMenu,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx) -> SandboxMode {
        SandboxMode {
            speed: SpeedControls::new(ctx, None),
            agent_tools: AgentTools::new(),
            time_travel: time_travel::InactiveTimeTravel::new(),
            common: CommonState::new(),
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
                        (hotkey(Key::F), "stop following agent"),
                        (hotkey(Key::R), "stop showing agent's route"),
                        (hotkey(Key::A), "show/hide active traffic"),
                        (hotkey(Key::T), "start time traveling"),
                        (hotkey(Key::Q), "scoreboard"),
                    ],
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (lctrl(Key::D), "debug mode"),
                        (lctrl(Key::E), "edit mode"),
                        (hotkey(Key::J), "warp"),
                        (hotkey(Key::K), "navigate"),
                        (hotkey(Key::Semicolon), "show/hide delayed traffic"),
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

        let mut txt = Text::prompt("Sandbox Mode");
        txt.add_line(ui.primary.sim.summary());
        self.agent_tools.update_menu_info(&mut txt);
        self.menu.handle_event(ctx, Some(txt));

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

        self.agent_tools.event(ctx, ui, &mut self.menu);
        if ui.primary.current_selection.is_none() && self.menu.action("start time traveling") {
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
            ui.recalculate_current_selection(ctx);
        }

        if self.speed.is_paused() {
            if !ui.primary.sim.is_empty() && self.menu.action("reset sim") {
                ui.primary.reset_sim();
                return Transition::Replace(Box::new(SandboxMode::new(ctx)));
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
        ui.draw(
            g,
            self.common.draw_options(ui),
            &ui.primary.sim,
            &ShowEverything::new(),
        );
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

    let (ss, _) = wiz.wrap(ctx).choose_something_no_keys::<()>(
        "Load which savestate?",
        Box::new(move || {
            abstutil::list_dir(std::path::Path::new(&path))
                .into_iter()
                .map(|f| (f, ()))
                .collect()
        }),
    )?;

    ctx.loading_screen("load savestate", |ctx, mut timer| {
        ui.primary.sim = Sim::load_savestate(ss, &mut timer).expect("Can't load savestate");
        ui.recalculate_current_selection(ctx);
    });
    Some(Transition::Pop)
}

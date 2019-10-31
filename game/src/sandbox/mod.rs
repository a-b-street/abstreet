mod analytics;
mod bus_explorer;
mod gameplay;
mod score;
mod spawner;
mod trip_stats;

use crate::common::{time_controls, AgentTools, CommonState, SpeedControls};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::ui::{ShowEverything, UI};
use ezgui::{
    hotkey, layout, lctrl, Choice, EventCtx, EventLoopMode, GfxCtx, Key, Line, MenuUnderButton,
    ModalMenu, Text, Wizard,
};
pub use gameplay::GameplayMode;
use geom::Duration;
use sim::Sim;

pub struct SandboxMode {
    speed: SpeedControls,
    info_tools: MenuUnderButton,
    general_tools: MenuUnderButton,
    save_tools: MenuUnderButton,
    agent_tools: AgentTools,
    analytics: analytics::Analytics,
    gameplay: gameplay::GameplayState,
    common: CommonState,
    menu: ModalMenu,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI, mode: GameplayMode) -> SandboxMode {
        SandboxMode {
            speed: SpeedControls::new(ctx, true),
            general_tools: MenuUnderButton::new(
                "assets/ui/hamburger.png",
                "General",
                vec![
                    (hotkey(Key::Escape), "quit"),
                    (lctrl(Key::D), "debug mode"),
                    (lctrl(Key::E), "edit mode"),
                    (hotkey(Key::F1), "take a screenshot"),
                ],
                0.2,
                ctx,
            ),
            info_tools: MenuUnderButton::new(
                "assets/ui/info.png",
                "Info",
                vec![
                    (hotkey(Key::Q), "scoreboard"),
                    (hotkey(Key::L), "change analytics overlay"),
                    (hotkey(Key::Semicolon), "change agent colorscheme"),
                    (None, "explore a bus route"),
                ],
                0.3,
                ctx,
            ),
            save_tools: MenuUnderButton::new(
                "assets/ui/save.png",
                "Savestates",
                vec![
                    (hotkey(Key::O), "save sim state"),
                    (hotkey(Key::Y), "load previous sim state"),
                    (hotkey(Key::U), "load next sim state"),
                    (None, "pick a savestate to load"),
                ],
                0.35,
                ctx,
            ),
            agent_tools: AgentTools::new(),
            analytics: analytics::Analytics::Inactive,
            gameplay: gameplay::GameplayState::initialize(mode, ui, ctx),
            common: CommonState::new(ctx),
            menu: ModalMenu::new("Sandbox Mode", vec![(hotkey(Key::X), "reset sim")], ctx)
                .disable_standalone_layout(),
        }
    }
}

impl State for SandboxMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        layout::stack_vertically(
            layout::ContainerOrientation::TopRight,
            ctx.canvas,
            vec![&mut self.menu, &mut self.gameplay.menu],
        );

        {
            let mut txt = Text::new();
            txt.add(Line(ui.primary.sim.time().to_string()));
            let (active, unfinished, buses) = ui.primary.sim.num_trips();
            txt.add(Line(format!("{} active (+{} buses)", active, buses)));
            txt.add(Line(format!("{} unfinished", unfinished)));
            self.menu.set_info(ctx, txt);
        }
        self.menu.event(ctx);
        self.info_tools.event(ctx);
        self.general_tools.event(ctx);
        self.save_tools.event(ctx);

        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        if let Some(t) = self.common.event(ctx, ui) {
            return t;
        }
        if let Some(t) = self.analytics.event(ctx, ui, &mut self.info_tools) {
            return t;
        }
        if let Some(t) = self.gameplay.event(ctx, ui, &mut self.analytics) {
            return t;
        }

        if let Some(t) = self
            .agent_tools
            .event(ctx, ui, &mut self.menu, &mut self.info_tools)
        {
            return t;
        }
        if self.info_tools.action("scoreboard") {
            return Transition::Push(Box::new(score::Scoreboard::new(ctx, ui)));
        }
        if let Some(explorer) = bus_explorer::BusRouteExplorer::new(ctx, ui) {
            return Transition::PushWithMode(explorer, EventLoopMode::Animation);
        }
        if let Some(picker) = bus_explorer::BusRoutePicker::new(ui, &mut self.info_tools) {
            return Transition::Push(picker);
        }

        if self.general_tools.action("quit") {
            return Transition::Pop;
        }
        if self.general_tools.action("debug mode") {
            return Transition::Push(Box::new(DebugMode::new(ctx, ui)));
        }
        if self.general_tools.action("edit mode") {
            return Transition::Replace(Box::new(EditMode::new(ctx, ui)));
        }
        if self.general_tools.action("take a screenshot") {
            return Transition::KeepWithMode(EventLoopMode::ScreenCaptureCurrentShot);
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

        if self.save_tools.action("save sim state") {
            self.speed.pause();
            ctx.loading_screen("savestate", |_, timer| {
                timer.start("save sim state");
                ui.primary.sim.save();
                timer.stop("save sim state");
            });
        }
        if self.save_tools.action("load previous sim state") {
            self.speed.pause();
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
        if self.save_tools.action("load next sim state") {
            self.speed.pause();
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
        if self.save_tools.action("pick a savestate to load") {
            self.speed.pause();
            return Transition::Push(WizardState::new(Box::new(load_savestate)));
        }

        if let Some(dt) = self.speed.event(ctx, ui.primary.sim.time()) {
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
                return Transition::Replace(Box::new(SandboxMode::new(
                    ctx,
                    ui,
                    self.gameplay.mode.clone(),
                )));
            }

            if let Some(t) = time_controls(ctx, ui, &mut self.speed) {
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
        if self.analytics.draw(g, ui) {
            // Don't draw agent tools!
        } else {
            ui.draw(
                g,
                self.common.draw_options(ui),
                &ui.primary.sim,
                &ShowEverything::new(),
            );
            self.agent_tools.draw(g, ui);
        }
        self.common.draw(g, ui);
        self.menu.draw(g);
        self.speed.draw(g);
        self.info_tools.draw(g);
        self.general_tools.draw(g);
        self.save_tools.draw(g);
        self.gameplay.draw(g);
    }

    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut UI) {
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

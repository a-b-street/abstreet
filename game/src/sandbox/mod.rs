mod bus_explorer;
mod gameplay;
mod overlays;
mod score;

use self::overlays::Overlays;
use crate::common::{time_controls, AgentTools, CommonState, SpeedControls};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::edit::{apply_map_edits, save_edits};
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::pregame::main_menu;
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, layout, lctrl, Choice, EventCtx, EventLoopMode, GfxCtx, Key, Line, MenuUnderButton,
    ModalMenu, Text, Wizard,
};
pub use gameplay::spawner::spawn_agents_around;
pub use gameplay::GameplayMode;
use geom::Duration;
use map_model::MapEdits;
use sim::Sim;

pub struct SandboxMode {
    speed: SpeedControls,
    info_tools: MenuUnderButton,
    general_tools: MenuUnderButton,
    save_tools: MenuUnderButton,
    agent_tools: AgentTools,
    overlay: Overlays,
    gameplay: gameplay::GameplayRunner,
    common: CommonState,
    menu: ModalMenu,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI, mode: GameplayMode) -> SandboxMode {
        SandboxMode {
            speed: SpeedControls::new(ctx, ui.primary.current_flags.dev, true),
            general_tools: MenuUnderButton::new(
                "assets/ui/hamburger.png",
                "General",
                vec![
                    (hotkey(Key::Escape), "back to title screen"),
                    (lctrl(Key::D), "debug mode"),
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
            overlay: Overlays::Inactive,
            gameplay: gameplay::GameplayRunner::initialize(mode, ui, ctx),
            common: CommonState::new(ctx),
            menu: ModalMenu::new(
                "Sandbox Mode",
                vec![(lctrl(Key::E), "edit mode"), (hotkey(Key::X), "reset sim")],
                ctx,
            )
            .disable_standalone_layout(),
        }
    }
}

impl State for SandboxMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        {
            let mut txt = Text::new();
            txt.add(Line(format!(
                "Time: {}",
                ui.primary.sim.time().ampm_tostring()
            )));
            let (active, unfinished, buses) = ui.primary.sim.num_trips();
            txt.add(Line(format!("{} active (+{} buses)", active, buses)));
            txt.add(Line(format!("{} unfinished", unfinished)));
            txt.add(Line(""));
            {
                let edits = ui.primary.map.get_edits();
                txt.add(Line(format!("Edits: {}", edits.edits_name)));
                if edits.dirty {
                    txt.append(Line("*"));
                }
            }
            self.menu.set_info(ctx, txt);
        }
        if let Some(t) = self.gameplay.event(ctx, ui, &mut self.overlay) {
            return t;
        }
        // Give both menus a chance to set_info before doing this
        layout::stack_vertically(
            layout::ContainerOrientation::TopRight,
            ctx,
            vec![&mut self.menu, &mut self.gameplay.menu],
        );

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
        if let Some(t) = self.overlay.event(ctx, ui, &mut self.info_tools) {
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

        if self.general_tools.action("back to title screen") {
            // TODO Clear edits?
            return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, ui| {
                let mut wizard = wiz.wrap(ctx);
                let dirty = ui.primary.map.get_edits().dirty;
                let (resp, _) =
                    wizard.choose("Sure you want to abandon the current challenge?", || {
                        let mut choices = Vec::new();
                        choices.push(Choice::new("keep playing", ()));
                        if dirty {
                            choices.push(Choice::new("save edits and quit", ()));
                        }
                        choices.push(Choice::new("quit challenge", ()).key(Key::Q));
                        choices
                    })?;
                let map_name = ui.primary.map.get_name().to_string();
                match resp.as_str() {
                    "save edits and quit" => {
                        save_edits(&mut wizard, ui)?;

                        // Always reset edits if we just saved edits.
                        apply_map_edits(&mut ui.primary, &ui.cs, ctx, MapEdits::new(map_name));
                        ui.primary.map.mark_edits_fresh();
                        ui.primary
                            .map
                            .recalculate_pathfinding_after_edits(&mut Timer::new("reset edits"));
                        ui.primary.clear_sim();
                        Some(Transition::Clear(main_menu(ctx, ui)))
                    }
                    "quit challenge" => {
                        if !ui.primary.map.get_edits().is_empty() {
                            apply_map_edits(&mut ui.primary, &ui.cs, ctx, MapEdits::new(map_name));
                            ui.primary.map.mark_edits_fresh();
                            ui.primary
                                .map
                                .recalculate_pathfinding_after_edits(&mut Timer::new(
                                    "reset edits",
                                ));
                        }
                        ui.primary.clear_sim();
                        Some(Transition::Clear(main_menu(ctx, ui)))
                    }
                    "keep playing" => Some(Transition::Pop),
                    _ => unreachable!(),
                }
            })));
        }
        if self.general_tools.action("debug mode") {
            return Transition::Push(Box::new(DebugMode::new(ctx, ui)));
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
        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            if ctx
                .input
                .contextual_action(Key::T, "throughput over 1-hour buckets")
            {
                let r = ui.primary.map.get_l(l).parent;
                let bucket = Duration::minutes(60);
                self.overlay = Overlays::road_throughput(r, bucket, ctx, ui);
            }
        }
        if let Some(ID::Intersection(i)) = ui.primary.current_selection {
            if ctx
                .input
                .contextual_action(Key::T, "throughput over 1-hour buckets")
            {
                let bucket = Duration::minutes(60);
                self.overlay = Overlays::intersection_throughput(i, bucket, ctx, ui);
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
            if let Some(t) = ctx.loading_screen("load previous savestate", |ctx, mut timer| {
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
                        None
                    }
                    None => Some(Transition::Push(msg(
                        "Error",
                        vec![format!("Couldn't load previous savestate {:?}", prev_state)],
                    ))),
                }
            }) {
                return t;
            }
        }
        if self.save_tools.action("load next sim state") {
            self.speed.pause();
            if let Some(t) = ctx.loading_screen("load next savestate", |ctx, mut timer| {
                let next_state = ui.primary.sim.find_next_savestate(ui.primary.sim.time());
                match next_state
                    .clone()
                    .and_then(|path| Sim::load_savestate(path, &mut timer).ok())
                {
                    Some(new_sim) => {
                        ui.primary.sim = new_sim;
                        ui.recalculate_current_selection(ctx);
                        None
                    }
                    None => Some(Transition::Push(msg(
                        "Error",
                        vec![format!("Couldn't load next savestate {:?}", next_state)],
                    ))),
                }
            }) {
                return t;
            }
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
        if let Some(t) = time_controls(ctx, ui, &mut self.speed) {
            return t;
        }

        if self.menu.action("edit mode") {
            ui.primary.clear_sim();
            return Transition::Replace(Box::new(EditMode::new(ctx, self.gameplay.mode.clone())));
        }
        if self.speed.is_paused() {
            if !ui.primary.sim.is_empty() && self.menu.action("reset sim") {
                ui.primary.clear_sim();
                return Transition::Replace(Box::new(SandboxMode::new(
                    ctx,
                    ui,
                    self.gameplay.mode.clone(),
                )));
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
        if self.overlay.draw(g, ui) {
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
        self.speed.draw(g, ui);
        self.info_tools.draw(g);
        self.general_tools.draw(g);
        self.save_tools.draw(g);
        self.gameplay.draw(g, ui);
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

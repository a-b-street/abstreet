mod bus_explorer;
mod gameplay;
mod overlays;
mod score;
mod speed;

use self::overlays::Overlays;
use crate::common::{tool_panel, AgentTools, CommonState, Minimap};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::edit::{apply_map_edits, save_edits};
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{Composite, ManagedWidget, Outcome};
use crate::pregame::main_menu;
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, layout, lctrl, Choice, Color, EventCtx, EventLoopMode, GfxCtx, Key, Line, ModalMenu,
    ScreenPt, Text,
};
pub use gameplay::spawner::spawn_agents_around;
pub use gameplay::GameplayMode;
use geom::{Duration, Time};
use map_model::MapEdits;
use sim::TripMode;

pub struct SandboxMode {
    speed: speed::SpeedControls,
    agent_meter: AgentMeter,
    agent_tools: AgentTools,
    overlay: Overlays,
    gameplay: gameplay::GameplayRunner,
    common: CommonState,
    tool_panel: Composite,
    minimap: Option<Minimap>,
    menu: ModalMenu,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI, mode: GameplayMode) -> SandboxMode {
        SandboxMode {
            speed: speed::SpeedControls::new(ctx, ui),
            agent_meter: AgentMeter::new(ctx, ui),
            agent_tools: AgentTools::new(),
            overlay: Overlays::Inactive,
            common: CommonState::new(),
            tool_panel: tool_panel(ctx, Some(Box::new(Overlays::change_overlays))),
            minimap: if mode.has_minimap() {
                Some(Minimap::new())
            } else {
                None
            },
            gameplay: gameplay::GameplayRunner::initialize(mode, ui, ctx),
            menu: ModalMenu::new(
                "Sandbox Mode",
                vec![
                    (lctrl(Key::E), "edit mode"),
                    (hotkey(Key::Q), "scoreboard"),
                    (hotkey(Key::Semicolon), "change agent colorscheme"),
                    (None, "explore a bus route"),
                ],
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
            let edits = ui.primary.map.get_edits();
            txt.add(Line(format!("Edits: {}", edits.edits_name)));
            if edits.dirty {
                txt.append(Line("*"));
            }
            self.menu.set_info(ctx, txt);
        }
        self.agent_meter.event(ctx, ui);
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

        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        if let Some(ref mut m) = self.minimap {
            m.event(ui, ctx);
        }

        if let Some(t) = self.agent_tools.event(ctx, ui, &mut self.menu) {
            return t;
        }
        if self.menu.action("scoreboard") {
            return Transition::Push(Box::new(score::Scoreboard::new(
                ctx,
                ui,
                &self.gameplay.prebaked,
            )));
        }
        if let Some(explorer) = bus_explorer::BusRouteExplorer::new(ctx, ui) {
            return Transition::PushWithMode(explorer, EventLoopMode::Animation);
        }
        if let Some(picker) = bus_explorer::BusRoutePicker::new(ui, &mut self.menu) {
            return Transition::Push(picker);
        }

        if ui.opts.dev && ctx.input.new_was_pressed(lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx)));
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
                && ui.per_obj.action(
                    ctx,
                    Key::P,
                    format!("examine {} cars parked here", cars.len()),
                )
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
            if ui
                .per_obj
                .action(ctx, Key::T, "throughput over 1-hour buckets")
            {
                let r = ui.primary.map.get_l(l).parent;
                let bucket = Duration::hours(1);
                self.overlay = Overlays::road_throughput(r, bucket, ctx, ui);
            }
        }
        if let Some(ID::Intersection(i)) = ui.primary.current_selection {
            if ui
                .per_obj
                .action(ctx, Key::T, "throughput over 1-hour buckets")
            {
                let bucket = Duration::hours(1);
                self.overlay = Overlays::intersection_throughput(i, bucket, ctx, ui);
            } else if ui.per_obj.action(ctx, Key::D, "delay over 1-hour buckets") {
                let bucket = Duration::hours(1);
                self.overlay = Overlays::intersection_delay_over_time(i, bucket, ctx, ui);
            } else if ui.primary.map.get_i(i).is_traffic_signal()
                && ui.per_obj.action(ctx, Key::E, "show current demand")
            {
                self.overlay = Overlays::intersection_demand(i, ctx, ui);
            }
        }

        if let Some(t) = self.speed.event(ctx, ui, &self.gameplay.mode) {
            return t;
        }

        if self.menu.action("edit mode") {
            ui.primary.clear_sim();
            return Transition::Replace(Box::new(EditMode::new(ctx, self.gameplay.mode.clone())));
        }

        if let Some(t) = self.common.event(ctx, ui) {
            return t;
        }
        if let Some(t) = self.overlay.event(ctx, ui, &self.gameplay.prebaked) {
            return t;
        }
        match self.tool_panel.event(ctx, ui) {
            Some(Outcome::Transition(t)) => {
                return t;
            }
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "back" => {
                    // TODO Clear edits?
                    return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, ui| {
                        let mut wizard = wiz.wrap(ctx);
                        let dirty = ui.primary.map.get_edits().dirty;
                        let (resp, _) = wizard.choose(
                            "Sure you want to abandon the current challenge?",
                            || {
                                let mut choices = Vec::new();
                                choices.push(Choice::new("keep playing", ()));
                                if dirty {
                                    choices.push(Choice::new("save edits and quit", ()));
                                }
                                choices.push(Choice::new("quit challenge", ()).key(Key::Q));
                                choices
                            },
                        )?;
                        let map_name = ui.primary.map.get_name().to_string();
                        match resp.as_str() {
                            "save edits and quit" => {
                                save_edits(&mut wizard, ui)?;

                                // Always reset edits if we just saved edits.
                                apply_map_edits(
                                    &mut ui.primary,
                                    &ui.cs,
                                    ctx,
                                    MapEdits::new(map_name),
                                );
                                ui.primary.map.mark_edits_fresh();
                                ui.primary.map.recalculate_pathfinding_after_edits(
                                    &mut Timer::new("reset edits"),
                                );
                                ui.primary.clear_sim();
                                Some(Transition::Clear(main_menu(ctx, ui)))
                            }
                            "quit challenge" => {
                                if !ui.primary.map.get_edits().is_empty() {
                                    apply_map_edits(
                                        &mut ui.primary,
                                        &ui.cs,
                                        ctx,
                                        MapEdits::new(map_name),
                                    );
                                    ui.primary.map.mark_edits_fresh();
                                    ui.primary.map.recalculate_pathfinding_after_edits(
                                        &mut Timer::new("reset edits"),
                                    );
                                }
                                ui.primary.clear_sim();
                                Some(Transition::Clear(main_menu(ctx, ui)))
                            }
                            "keep playing" => Some(Transition::Pop),
                            _ => unreachable!(),
                        }
                    })));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if self.speed.is_paused() {
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
        self.tool_panel.draw(g);
        self.menu.draw(g);
        self.speed.draw(g);
        self.gameplay.draw(g, ui);
        self.agent_meter.draw(g);
        if let Some(ref m) = self.minimap {
            m.draw(g, ui);
        }
    }

    fn on_suspend(&mut self, _: &mut EventCtx, _: &mut UI) {
        self.speed.pause();
    }
}

struct AgentMeter {
    time: Time,
    composite: Composite,
}

impl AgentMeter {
    pub fn new(ctx: &EventCtx, ui: &UI) -> AgentMeter {
        let (active, unfinished, by_mode) = ui.primary.sim.num_trips();

        let composite = Composite::minimal_size(
            ManagedWidget::col(vec![
                {
                    let mut txt = Text::new();
                    txt.add(Line(format!("Active trips: {}", active)));
                    txt.add(Line(format!("Unfinished trips: {}", unfinished)));
                    ManagedWidget::draw_text(ctx, txt)
                },
                ManagedWidget::row(vec![
                    ManagedWidget::draw_svg(ctx, "assets/meters/pedestrian.svg"),
                    ManagedWidget::draw_text(ctx, Text::from(Line(&by_mode[&TripMode::Walk]))),
                    ManagedWidget::draw_svg(ctx, "assets/meters/bike.svg"),
                    ManagedWidget::draw_text(ctx, Text::from(Line(&by_mode[&TripMode::Bike]))),
                    ManagedWidget::draw_svg(ctx, "assets/meters/car.svg"),
                    ManagedWidget::draw_text(ctx, Text::from(Line(&by_mode[&TripMode::Drive]))),
                    ManagedWidget::draw_svg(ctx, "assets/meters/bus.svg"),
                    ManagedWidget::draw_text(ctx, Text::from(Line(&by_mode[&TripMode::Transit]))),
                ])
                .centered(),
            ])
            .bg(Color::grey(0.4))
            .padding(20),
            ScreenPt::new(350.0, 10.0),
        );

        AgentMeter {
            time: ui.primary.sim.time(),
            composite,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) {
        if self.time != ui.primary.sim.time() {
            *self = AgentMeter::new(ctx, ui);
        }
        self.composite.event(ctx, ui);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}

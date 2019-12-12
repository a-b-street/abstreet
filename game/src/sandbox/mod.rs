mod bus_explorer;
mod gameplay;
mod overlays;
mod score;
mod speed;

use self::overlays::Overlays;
use crate::common::{AgentTools, CommonState, Minimap};
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::edit::{apply_map_edits, save_edits};
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::pregame::main_menu;
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::layout::Widget;
use ezgui::{
    hotkey, layout, lctrl, Choice, Color, DrawBoth, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    JustDraw, Key, Line, ModalMenu, ScreenDims, ScreenPt, ScreenRectangle, Text,
};
pub use gameplay::spawner::spawn_agents_around;
pub use gameplay::GameplayMode;
use geom::{Distance, Duration, Polygon, Time};
use map_model::MapEdits;
use sim::TripMode;

pub struct SandboxMode {
    speed: speed::SpeedControls,
    agent_meter: AgentMeter,
    agent_tools: AgentTools,
    overlay: Overlays,
    gameplay: gameplay::GameplayRunner,
    common: CommonState,
    minimap: Option<Minimap>,
    menu: ModalMenu,
}

impl SandboxMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI, mode: GameplayMode) -> SandboxMode {
        SandboxMode {
            speed: speed::SpeedControls::new(ctx, ui.opts.dev),
            agent_meter: AgentMeter::new(ctx, ui),
            agent_tools: AgentTools::new(),
            overlay: Overlays::Inactive,
            common: CommonState::new(ctx, true),
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
        if let Some(t) = self.overlay.event(
            ctx,
            ui,
            self.common.tool_panel.layers_btn.as_mut().unwrap(),
            &self.gameplay.prebaked,
        ) {
            return t;
        }
        if self.common.tool_panel.home_btn.clicked() {
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
        self.menu.draw(g);
        self.speed.draw(g, ui);
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

// TODO Some kind of composite thing...
struct AgentMeter {
    time: Time,
    widgets: Vec<JustDraw>,
    rect: ScreenRectangle,
}

impl AgentMeter {
    pub fn new(ctx: &EventCtx, ui: &UI) -> AgentMeter {
        let (active, unfinished, by_mode) = ui.primary.sim.num_trips();

        let mut row1_txt = Text::new().no_bg();
        row1_txt.add(Line(format!("Active trips: {}", active)));
        row1_txt.add(Line(format!("Unfinished trips: {}", unfinished)));

        // TODO Hardcoding guessed dims
        let rect_bg = GeomBatch::from(vec![(
            Color::grey(0.4),
            Polygon::rounded_rectangle(
                Distance::meters(290.0),
                Distance::meters(100.0),
                Distance::meters(5.0),
            ),
        )]);

        // TODO Rectangle behind everything
        let mut widgets = vec![
            JustDraw::wrap(DrawBoth::new(ctx, rect_bg, Vec::new())),
            JustDraw::text(row1_txt, ctx),
            JustDraw::svg("assets/meters/pedestrian.svg", ctx),
            JustDraw::text(Text::from(Line(&by_mode[&TripMode::Walk])).no_bg(), ctx),
            JustDraw::svg("assets/meters/bike.svg", ctx),
            JustDraw::text(Text::from(Line(&by_mode[&TripMode::Bike])).no_bg(), ctx),
            JustDraw::svg("assets/meters/car.svg", ctx),
            JustDraw::text(Text::from(Line(&by_mode[&TripMode::Drive])).no_bg(), ctx),
            JustDraw::svg("assets/meters/bus.svg", ctx),
            JustDraw::text(Text::from(Line(&by_mode[&TripMode::Transit])).no_bg(), ctx),
        ];

        // TODO A horrible experiment in manual layouting

        let top_left = ScreenPt::new(ctx.canvas.window_width - 300.0, 350.0);
        widgets[0].set_pos(top_left);
        widgets[1].set_pos(top_left);
        let top_left = ScreenPt::new(top_left.x, top_left.y + widgets[1].get_dims().height);
        layout::stack_horizontally(
            top_left,
            // TODO Padding is wrong, want to alternate the amount
            5.0,
            widgets
                .iter_mut()
                .skip(2)
                .map(|w| w as &mut dyn Widget)
                .collect(),
        );
        AgentMeter {
            widgets,
            time: ui.primary.sim.time(),
            rect: ScreenRectangle::top_left(top_left, ScreenDims::new(290.0, 100.0)),
        }
    }

    pub fn event(&mut self, ctx: &EventCtx, ui: &UI) {
        // TODO Or window size changed...
        if self.time != ui.primary.sim.time() {
            *self = AgentMeter::new(ctx, ui);
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        for w in &self.widgets {
            w.draw(g);
        }
        g.canvas.mark_covered_area(self.rect.clone());
    }
}

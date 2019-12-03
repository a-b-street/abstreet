mod lanes;
mod stop_signs;
mod traffic_signals;

use crate::common::{CommonState, Warping};
use crate::debug::DebugMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::{ColorScheme, ID};
use crate::options;
use crate::render::{
    DrawIntersection, DrawLane, DrawOptions, DrawRoad, Renderable, MIN_ZOOM_FOR_DETAIL,
};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::{PerMapUI, ShowEverything, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Choice, Color, EventCtx, EventLoopMode, GfxCtx, Key, Line, MenuUnderButton,
    ModalMenu, Text, Wizard, WrappedWizard,
};
use map_model::{ControlStopSign, ControlTrafficSignal, EditCmd, LaneID, MapEdits};
use std::collections::BTreeSet;

pub struct EditMode {
    common: CommonState,
    menu: ModalMenu,
    general_tools: MenuUnderButton,
    mode: GameplayMode,

    lane_editor: lanes::LaneEditor,
}

impl EditMode {
    pub fn new(ctx: &EventCtx, mode: GameplayMode) -> EditMode {
        EditMode {
            common: CommonState::new(ctx),
            menu: ModalMenu::new(
                "Map Edit Mode",
                vec![
                    (hotkey(Key::Escape), "back to sandbox mode"),
                    (hotkey(Key::S), "save edits"),
                    (hotkey(Key::L), "load different edits"),
                    (lctrl(Key::Z), "undo"),
                    (hotkey(Key::Num1), "1) ..."),
                    (hotkey(Key::Num2), "2) ..."),
                    (hotkey(Key::Num3), "3) ..."),
                    (hotkey(Key::Num4), "4) ..."),
                    (hotkey(Key::Num5), "5) ..."),
                ],
                ctx,
            ),
            general_tools: MenuUnderButton::new(
                "assets/ui/hamburger.png",
                "General",
                vec![
                    (lctrl(Key::D), "debug mode"),
                    (hotkey(Key::F1), "take a screenshot"),
                    (None, "options"),
                ],
                0.2,
                ctx,
            ),
            mode,
            lane_editor: lanes::LaneEditor::setup(ctx),
        }
    }
}

impl State for EditMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        {
            let edits = ui.primary.map.get_edits();
            let mut txt = Text::new();
            txt.add(Line(format!("Edits: {}", edits.edits_name)));
            if edits.dirty {
                txt.append(Line("*"));
            }
            txt.add(Line(format!(
                "{} lane types changed",
                edits.original_lts.len()
            )));
            txt.add(Line(format!(
                "{} lanes reversed",
                edits.reversed_lanes.len()
            )));
            txt.add(Line(format!(
                "{} intersections changed",
                edits.changed_intersections.len()
            )));
            self.menu.set_info(ctx, txt);
        }

        // TODO Recalculate less frequently
        {
            let cmds = &ui.primary.map.get_edits().commands;
            for (idx, key) in vec![Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5]
                .into_iter()
                .enumerate()
            {
                let label = if idx < cmds.len() {
                    format!("{}) {}", idx + 1, cmds[cmds.len() - 1 - idx].describe())
                } else {
                    format!("{}) ...", idx + 1)
                };
                self.menu
                    .change_action_by_key(hotkey(key).unwrap(), label, ctx);
            }
        }

        self.menu.event(ctx);
        self.general_tools.event(ctx);

        {
            let cmds = &ui.primary.map.get_edits().commands;
            for idx in 1..=5 {
                if idx <= cmds.len() {
                    let label = format!("{}) {}", idx, cmds[cmds.len() - idx].describe());
                    if self.menu.action(&label) {
                        let id = match &cmds[cmds.len() - idx] {
                            EditCmd::ChangeLaneType { id, .. } => ID::Lane(*id),
                            EditCmd::ReverseLane { l, .. } => ID::Lane(*l),
                            EditCmd::ChangeStopSign(ss) => ID::Intersection(ss.id),
                            EditCmd::ChangeTrafficSignal(ss) => ID::Intersection(ss.id),
                            EditCmd::CloseIntersection { id, .. } => ID::Intersection(*id),
                            EditCmd::UncloseIntersection(id, _) => ID::Intersection(*id),
                        };
                        return Transition::PushWithMode(
                            Warping::new(
                                ctx,
                                id.canonical_point(&ui.primary).unwrap(),
                                None,
                                Some(id),
                                &mut ui.primary,
                            ),
                            EventLoopMode::Animation,
                        );
                    }
                }
            }
        }

        if self.mode.can_edit_lanes() {
            if let Some(t) = self.lane_editor.event(ui, ctx) {
                return t;
            }
        }
        ctx.canvas.handle_event(ctx.input);
        // It only makes sense to mouseover lanes while painting them.
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
            if let Some(ID::Lane(_)) = ui.primary.current_selection {
            } else if let Some(ID::Intersection(_)) = ui.primary.current_selection {
                if self.lane_editor.active_idx != Some(self.lane_editor.construction_idx)
                    && self.lane_editor.active_idx.is_some()
                {
                    ui.primary.current_selection = None;
                }
            } else {
                if self.lane_editor.active_idx.is_some() {
                    ui.primary.current_selection = None;
                }
            }
        }

        if let Some(t) = self.common.event(ctx, ui) {
            return t;
        }

        if self.general_tools.action("debug mode") {
            return Transition::Push(Box::new(DebugMode::new(ctx, ui)));
        }
        if self.general_tools.action("take a screenshot") {
            return Transition::KeepWithMode(EventLoopMode::ScreenCaptureCurrentShot);
        }
        if self.general_tools.action("options") {
            return Transition::Push(options::open_panel());
        }

        if ui.primary.map.get_edits().dirty && self.menu.action("save edits") {
            return Transition::Push(WizardState::new(Box::new(|wiz, ctx, ui| {
                save_edits(&mut wiz.wrap(ctx), ui)?;
                Some(Transition::Pop)
            })));
        } else if self.menu.action("load different edits") {
            return Transition::Push(WizardState::new(Box::new(load_edits)));
        } else if self.menu.action("back to sandbox mode") {
            // TODO Maybe put a loading screen around these.
            ui.primary
                .map
                .recalculate_pathfinding_after_edits(&mut Timer::new("apply pending map edits"));
            // Parking state might've changed
            ui.primary.clear_sim();
            return Transition::Replace(Box::new(SandboxMode::new(ctx, ui, self.mode.clone())));
        }

        if let Some(ID::Intersection(id)) = ui.primary.current_selection {
            if ui.primary.map.maybe_get_stop_sign(id).is_some() {
                if self.mode.can_edit_stop_signs()
                    && ctx
                        .input
                        .contextual_action(Key::E, format!("edit stop signs for {}", id))
                {
                    return Transition::Push(Box::new(stop_signs::StopSignEditor::new(
                        id, ctx, ui,
                    )));
                } else if ui
                    .primary
                    .map
                    .get_edits()
                    .changed_intersections
                    .contains(&id)
                    && ctx.input.contextual_action(Key::R, "revert")
                {
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(EditCmd::ChangeStopSign(ControlStopSign::new(
                            &ui.primary.map,
                            id,
                        )));
                    apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
                }
            }
            if ui.primary.map.maybe_get_traffic_signal(id).is_some() {
                if ctx
                    .input
                    .contextual_action(Key::E, format!("edit traffic signal for {}", id))
                {
                    return Transition::Push(Box::new(traffic_signals::TrafficSignalEditor::new(
                        id, ctx, ui,
                    )));
                } else if ui
                    .primary
                    .map
                    .get_edits()
                    .changed_intersections
                    .contains(&id)
                    && ctx.input.contextual_action(Key::R, "revert")
                {
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(EditCmd::ChangeTrafficSignal(ControlTrafficSignal::new(
                            &ui.primary.map,
                            id,
                            &mut Timer::throwaway(),
                        )));
                    apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
                }
            }
            if ui.primary.map.get_i(id).is_closed() && ctx.input.contextual_action(Key::R, "revert")
            {
                let mut edits = ui.primary.map.get_edits().clone();
                edits
                    .commands
                    .push(EditCmd::UncloseIntersection(id, edits.original_it(id)));
                apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
            }
        }

        if !ui.primary.map.get_edits().commands.is_empty() && self.menu.action("undo") {
            let mut edits = ui.primary.map.get_edits().clone();
            edits.commands.pop();
            apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
        }

        Transition::Keep
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

        // More generally we might want to show the diff between two edits, but for now,
        // just show diff relative to basemap.
        let edits = ui.primary.map.get_edits();

        let ctx = ui.draw_ctx();
        let mut opts = DrawOptions::new();

        // TODO Similar to drawing areas with traffic or not -- would be convenient to just
        // supply a set of things to highlight and have something else take care of drawing
        // with detail or not.
        if g.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL {
            for l in edits.original_lts.keys().chain(&edits.reversed_lanes) {
                opts.override_colors
                    .insert(ID::Lane(*l), Color::HatchingStyle1);
                ctx.draw_map.get_l(*l).draw(g, &opts, &ctx);
            }
            for i in &edits.changed_intersections {
                opts.override_colors
                    .insert(ID::Intersection(*i), Color::HatchingStyle1);
                ctx.draw_map.get_i(*i).draw(g, &opts, &ctx);
            }

            // The hatching covers up the selection outline, so redraw it.
            match ui.primary.current_selection {
                Some(ID::Lane(l)) => {
                    g.draw_polygon(
                        ui.cs.get("selected"),
                        &ctx.draw_map.get_l(l).get_outline(&ctx.map),
                    );
                }
                Some(ID::Intersection(i)) => {
                    g.draw_polygon(
                        ui.cs.get("selected"),
                        &ctx.draw_map.get_i(i).get_outline(&ctx.map),
                    );
                }
                _ => {}
            }
        } else {
            let color = ui.cs.get_def("unzoomed map diffs", Color::RED);
            for l in edits.original_lts.keys().chain(&edits.reversed_lanes) {
                g.draw_polygon(color, &ctx.map.get_parent(*l).get_thick_polygon().unwrap());
            }

            for i in &edits.changed_intersections {
                opts.override_colors.insert(ID::Intersection(*i), color);
                ctx.draw_map.get_i(*i).draw(g, &opts, &ctx);
            }
        }

        self.common.draw(g, ui);
        self.menu.draw(g);
        self.general_tools.draw(g);
        if self.mode.can_edit_lanes() {
            self.lane_editor.draw(g);
        }
    }
}

pub fn save_edits(wizard: &mut WrappedWizard, ui: &mut UI) -> Option<()> {
    let map = &mut ui.primary.map;

    let rename = if map.get_edits().edits_name == "no_edits" {
        Some(wizard.input_string("Name these map edits")?)
    } else {
        None
    };
    // TODO Don't allow naming them no_edits!

    // TODO Do it this weird way to avoid saving edits on every event. :P
    // TODO Do some kind of versioning? Don't ask this if the file doesn't exist yet?
    let save = "save edits";
    let cancel = "cancel";
    if wizard
        .choose_string("Overwrite edits?", || vec![save, cancel])?
        .as_str()
        == save
    {
        if let Some(name) = rename {
            let mut edits = map.get_edits().clone();
            edits.edits_name = name;
            map.apply_edits(edits, &mut Timer::new("name map edits"));
        }
        map.save_edits();
    }
    Some(())
}

fn load_edits(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);

    if ui.primary.map.get_edits().dirty {
        let save = "save edits";
        let discard = "discard";
        if wizard
            .choose_string("Save current edits first?", || vec![save, discard])?
            .as_str()
            == save
        {
            save_edits(&mut wizard, ui)?;
            wizard.reset();
        }
    }

    // TODO Exclude current
    let map_name = ui.primary.map.get_name().to_string();
    let (_, new_edits) = wizard.choose("Load which map edits?", || {
        let mut list = Choice::from(abstutil::load_all_objects(abstutil::path_all_edits(
            &map_name,
        )));
        list.push(Choice::new("no_edits", MapEdits::new(map_name.clone())));
        list
    })?;
    apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
    ui.primary.map.mark_edits_fresh();
    Some(Transition::Pop)
}

pub fn apply_map_edits(
    bundle: &mut PerMapUI,
    cs: &ColorScheme,
    ctx: &mut EventCtx,
    mut edits: MapEdits,
) {
    edits.dirty = true;
    let mut timer = Timer::new("apply map edits");

    let (lanes_changed, roads_changed, turns_deleted, turns_added, mut modified_intersections) =
        bundle.map.apply_edits(edits, &mut timer);

    for l in lanes_changed {
        bundle.draw_map.lanes[l.0] = DrawLane::new(
            bundle.map.get_l(l),
            &bundle.map,
            bundle.current_flags.draw_lane_markings,
            cs,
            &mut timer,
        )
        .finish(ctx.prerender);
    }
    for r in roads_changed {
        bundle.draw_map.roads[r.0] =
            DrawRoad::new(bundle.map.get_r(r), &bundle.map, cs, ctx.prerender);
    }

    let mut lanes_of_modified_turns: BTreeSet<LaneID> = BTreeSet::new();
    for t in turns_deleted {
        lanes_of_modified_turns.insert(t.src);
        modified_intersections.insert(t.parent);
    }
    for t in &turns_added {
        lanes_of_modified_turns.insert(t.src);
        modified_intersections.insert(t.parent);
    }

    for i in modified_intersections {
        bundle.draw_map.intersections[i.0] = DrawIntersection::new(
            bundle.map.get_i(i),
            &bundle.map,
            cs,
            ctx.prerender,
            &mut timer,
        );
    }
}

mod lanes;
mod stop_signs;
mod traffic_signals;

pub use self::lanes::LaneEditor;
pub use self::stop_signs::StopSignEditor;
pub use self::traffic_signals::TrafficSignalEditor;
use crate::colors;
use crate::common::{tool_panel, Colorer, CommonState, Overlays, Warping};
use crate::debug::DebugMode;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::{DrawIntersection, DrawLane, DrawRoad, MIN_ZOOM_FOR_DETAIL};
use crate::sandbox::{GameplayMode, SandboxMode};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, ManagedWidget, Outcome, RewriteColor, ScreenRectangle, Text, VerticalAlignment,
    WrappedWizard,
};
use geom::Polygon;
use map_model::{
    connectivity, EditCmd, EditIntersection, IntersectionID, LaneID, LaneType, MapEdits,
    PathConstraints,
};
use sim::{DontDrawAgents, Sim};
use std::collections::BTreeSet;

pub struct EditMode {
    tool_panel: WrappedComposite,
    composite: Composite,

    // Retained state from the SandboxMode that spawned us
    mode: GameplayMode,
    pub suspended_sim: Sim,

    // edits name, number of commands
    top_panel_key: (String, usize),
    once: bool,
}

impl EditMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI, mode: GameplayMode) -> EditMode {
        let suspended_sim = ui.primary.clear_sim();
        let edits = ui.primary.map.get_edits();
        EditMode {
            tool_panel: tool_panel(ctx),
            composite: make_topcenter(ctx, ui),
            mode,
            suspended_sim,
            top_panel_key: (edits.edits_name.clone(), edits.commands.len()),
            once: true,
        }
    }

    fn quit(&self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.loading_screen("apply edits", |ctx, mut timer| {
            ui.overlay = Overlays::Inactive;
            ui.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
            // Parking state might've changed
            ui.primary.clear_sim();
            // Autosave
            if ui.primary.map.get_edits().edits_name != "untitled edits" {
                ui.primary.map.save_edits();
            }
            Transition::PopThenReplace(Box::new(SandboxMode::new(ctx, ui, self.mode.clone())))
        })
    }
}

impl State for EditMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // Can't do this in the constructor, because SandboxMode's on_destroy clears out Overlays
        if self.once {
            self.once = false;
            // apply_map_edits will do the job later
            ui.overlay = Overlays::map_edits(ctx, ui);
        }
        {
            let edits = ui.primary.map.get_edits();
            let top_panel_key = (edits.edits_name.clone(), edits.commands.len());
            if self.top_panel_key != top_panel_key {
                self.top_panel_key = top_panel_key;
                self.composite = make_topcenter(ctx, ui);
            }
        }

        ctx.canvas_movement();
        // Restrict what can be selected.
        if ctx.redo_mouseover() {
            ui.primary.current_selection = ui.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
            );
            if let Some(ID::Lane(l)) = ui.primary.current_selection {
                if !can_edit_lane(&self.mode, l, ui) {
                    ui.primary.current_selection = None;
                }
            } else if let Some(ID::Intersection(_)) = ui.primary.current_selection {
            } else if let Some(ID::Road(_)) = ui.primary.current_selection {
            } else {
                ui.primary.current_selection = None;
            }
        }

        if ui.opts.dev && ctx.input.new_was_pressed(lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx)));
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "load edits" => {
                    // Autosave first
                    if ui.primary.map.get_edits().edits_name != "untitled edits" {
                        ui.primary.map.save_edits();
                    }
                    return Transition::Push(make_load_edits(
                        self.composite.rect_of("load edits").clone(),
                        self.mode.clone(),
                    ));
                }
                "finish editing" => {
                    return self.quit(ctx, ui);
                }
                "save edits as" => {
                    return Transition::Push(WizardState::new(Box::new(|wiz, ctx, ui| {
                        save_edits_as(&mut wiz.wrap(ctx), ui)?;
                        Some(Transition::Pop)
                    })));
                }
                "undo" => {
                    let mut edits = ui.primary.map.get_edits().clone();
                    let id = match edits.commands.pop().unwrap() {
                        EditCmd::ChangeLaneType { id, .. } => ID::Lane(id),
                        EditCmd::ReverseLane { l, .. } => ID::Lane(l),
                        EditCmd::ChangeIntersection { i, .. } => ID::Intersection(i),
                    };
                    apply_map_edits(ctx, ui, edits);
                    return Transition::Push(Warping::new(
                        ctx,
                        id.canonical_point(&ui.primary).unwrap(),
                        None,
                        Some(id),
                        &mut ui.primary,
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            if let Some(id) = &ui.primary.current_selection {
                if ui.per_obj.left_click(ctx, "edit this") {
                    return Transition::Push(Warping::new(
                        ctx,
                        id.canonical_point(&ui.primary).unwrap(),
                        Some(10.0),
                        None,
                        &mut ui.primary,
                    ));
                }
            }
        } else {
            if let Some(ID::Intersection(id)) = ui.primary.current_selection {
                if ui.primary.map.maybe_get_stop_sign(id).is_some()
                    && self.mode.can_edit_stop_signs()
                    && ui.per_obj.left_click(ctx, "edit stop signs")
                {
                    return Transition::Push(Box::new(StopSignEditor::new(
                        id,
                        ctx,
                        ui,
                        self.suspended_sim.clone(),
                    )));
                }
                if ui.primary.map.maybe_get_traffic_signal(id).is_some()
                    && ui.per_obj.left_click(ctx, "edit traffic signal")
                {
                    return Transition::Push(Box::new(TrafficSignalEditor::new(
                        id,
                        ctx,
                        ui,
                        self.suspended_sim.clone(),
                    )));
                }
                if ui.primary.map.get_i(id).is_closed()
                    && ui.per_obj.left_click(ctx, "re-open closed intersection")
                {
                    // This resets to the original state; it doesn't undo the closure to the last
                    // state. Seems reasonable to me.
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeIntersection {
                        i: id,
                        old: ui.primary.map.get_i_edit(id),
                        new: edits.original_intersections[&id].clone(),
                    });
                    apply_map_edits(ctx, ui, edits);
                }
            }
            if let Some(ID::Lane(l)) = ui.primary.current_selection {
                if ui.per_obj.left_click(ctx, "edit lane") {
                    return Transition::Push(Box::new(LaneEditor::new(l, ctx, ui)));
                }
            }
        }

        match self.tool_panel.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "back" => self.quit(ctx, ui),
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // TODO Maybe this should be part of ui.draw
        // TODO This has an X button, but we never call update and allow it to be changed. Should
        // just omit the button.
        ui.overlay.draw(g);

        self.tool_panel.draw(g);
        self.composite.draw(g);
        CommonState::draw_osd(g, ui, &ui.primary.current_selection);
    }
}

pub fn save_edits_as(wizard: &mut WrappedWizard, ui: &mut UI) -> Option<()> {
    let map = &mut ui.primary.map;
    let new_default_name = if map.get_edits().edits_name == "untitled edits" {
        "".to_string()
    } else {
        format!("copy of {}", map.get_edits().edits_name)
    };

    let name = loop {
        let candidate = wizard.input_something(
            "Name the new copy of these edits",
            Some(new_default_name.clone()),
            Box::new(|l| {
                if l.contains("/") || l == "untitled edits" || l == "" {
                    None
                } else {
                    Some(l)
                }
            }),
        )?;
        if abstutil::file_exists(abstutil::path_edits(map.get_name(), &candidate)) {
            let overwrite = "Overwrite";
            let rename = "Rename";
            if wizard
                .choose_string(&format!("Edits named {} already exist", candidate), || {
                    vec![overwrite, rename]
                })?
                .as_str()
                == overwrite
            {
                break candidate;
            }
        } else {
            break candidate;
        }
    };

    let mut edits = map.get_edits().clone();
    edits.edits_name = name;
    map.apply_edits(edits, &mut Timer::new("name map edits"));
    map.save_edits();
    Some(())
}

fn make_load_edits(btn: ScreenRectangle, mode: GameplayMode) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let mut wizard = wiz.wrap(ctx);

        if ui.primary.map.get_edits().edits_name == "untitled edits"
            && !ui.primary.map.get_edits().commands.is_empty()
        {
            let save = "save edits";
            let discard = "discard";
            if wizard
                .choose_string("Save current edits first?", || vec![save, discard])?
                .as_str()
                == save
            {
                save_edits_as(&mut wizard, ui)?;
                wizard.reset();
            }
        }

        // TODO Exclude current
        let current_edits_name = ui.primary.map.get_edits().edits_name.clone();
        let map_name = ui.primary.map.get_name().clone();
        let (_, new_edits) = wizard.choose_exact(
            (
                HorizontalAlignment::Centered(btn.center().x),
                VerticalAlignment::Below(btn.y2 + 15.0),
            ),
            None,
            || {
                let mut list = Choice::from(
                    abstutil::load_all_objects(abstutil::path_all_edits(&map_name))
                        .into_iter()
                        .filter(|(_, edits)| {
                            mode.allows(edits) && edits.edits_name != current_edits_name
                        })
                        .collect(),
                );
                list.push(Choice::new(
                    "start over with blank edits",
                    MapEdits::new(map_name.clone()),
                ));
                list
            },
        )?;
        apply_map_edits(ctx, ui, new_edits);
        Some(Transition::Pop)
    }))
}

fn make_topcenter(ctx: &mut EventCtx, ui: &UI) -> Composite {
    // TODO Support redo. Bit harder here to reset the redo_stack when the edits
    // change, because nested other places modify it too.
    Composite::new(
        ManagedWidget::col(vec![
            ManagedWidget::row(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line("Editing map").size(26))).margin(5),
                ManagedWidget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 30.0))]),
                )
                .margin(5),
                WrappedComposite::nice_text_button(
                    ctx,
                    Text::from(
                        Line(format!("{} ▼", &ui.primary.map.get_edits().edits_name))
                            .size(18)
                            .roboto(),
                    ),
                    lctrl(Key::L),
                    "load edits",
                )
                .margin(5),
                WrappedComposite::svg_button(
                    ctx,
                    "../data/system/assets/tools/save.svg",
                    "save edits as",
                    lctrl(Key::S),
                )
                .margin(5),
                (if !ui.primary.map.get_edits().commands.is_empty() {
                    WrappedComposite::svg_button(
                        ctx,
                        "../data/system/assets/tools/undo.svg",
                        "undo",
                        lctrl(Key::Z),
                    )
                } else {
                    ManagedWidget::draw_svg_transform(
                        ctx,
                        "../data/system/assets/tools/undo.svg",
                        RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                    )
                })
                .margin(15),
            ])
            .centered(),
            WrappedComposite::text_button(ctx, "finish editing", hotkey(Key::Escape))
                .centered_horiz(),
        ])
        .bg(colors::PANEL_BG),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

pub fn apply_map_edits(ctx: &mut EventCtx, ui: &mut UI, edits: MapEdits) {
    let mut timer = Timer::new("apply map edits");

    let (lanes_changed, roads_changed, turns_deleted, turns_added, mut modified_intersections) =
        ui.primary.map.apply_edits(edits, &mut timer);

    for l in lanes_changed {
        let lane = ui.primary.map.get_l(l);
        ui.primary.draw_map.lanes[l.0] = DrawLane::new(
            lane,
            &ui.primary.map,
            ui.primary.current_flags.draw_lane_markings,
            &ui.cs,
            &mut timer,
        )
        .finish(ctx.prerender, lane);
    }
    for r in roads_changed {
        ui.primary.draw_map.roads[r.0] = DrawRoad::new(
            ui.primary.map.get_r(r),
            &ui.primary.map,
            &ui.cs,
            ctx.prerender,
        );
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
        ui.primary.draw_map.intersections[i.0] = DrawIntersection::new(
            ui.primary.map.get_i(i),
            &ui.primary.map,
            &ui.cs,
            ctx.prerender,
            &mut timer,
        );
    }

    if let Overlays::Edits(_) = ui.overlay {
        ui.overlay = Overlays::map_edits(ctx, ui);
    }
}

pub fn can_edit_lane(mode: &GameplayMode, l: LaneID, ui: &UI) -> bool {
    mode.can_edit_lanes()
        && !ui.primary.map.get_l(l).is_sidewalk()
        && ui.primary.map.get_l(l).lane_type != LaneType::SharedLeftTurn
}

pub fn close_intersection(
    ctx: &mut EventCtx,
    ui: &mut UI,
    i: IntersectionID,
    pop_once: bool,
) -> Transition {
    let mut edits = ui.primary.map.get_edits().clone();
    edits.commands.push(EditCmd::ChangeIntersection {
        i,
        old: ui.primary.map.get_i_edit(i),
        new: EditIntersection::Closed,
    });
    apply_map_edits(ctx, ui, edits);

    let (_, disconnected) = connectivity::find_scc(&ui.primary.map, PathConstraints::Pedestrian);
    if disconnected.is_empty() {
        // Success! Quit the stop sign / signal editor.
        if pop_once {
            return Transition::Pop;
        } else {
            return Transition::PopTwice;
        }
    }

    let mut edits = ui.primary.map.get_edits().clone();
    edits.commands.pop();
    apply_map_edits(ctx, ui, edits);

    let mut err_state = msg(
        "Error",
        vec![format!(
            "Can't close this intersection; {} sidewalks disconnected",
            disconnected.len()
        )],
    );

    let color = ui.cs.get("unreachable lane");
    let mut c = Colorer::new(Text::new(), vec![("", color)]);
    for l in disconnected {
        c.add_l(l, color, &ui.primary.map);
    }

    err_state.downcast_mut::<WizardState>().unwrap().also_draw = Some(c.build_zoomed(ctx, ui));
    if pop_once {
        Transition::Push(err_state)
    } else {
        Transition::Replace(err_state)
    }
}

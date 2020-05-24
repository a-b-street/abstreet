mod bulk;
mod cluster_traffic_signals;
mod lanes;
mod stop_signs;
mod traffic_signals;

pub use self::cluster_traffic_signals::ClusterTrafficSignalEditor;
pub use self::lanes::LaneEditor;
pub use self::stop_signs::StopSignEditor;
pub use self::traffic_signals::TrafficSignalEditor;
use crate::app::{App, ShowEverything};
use crate::common::{tool_panel, Colorer, CommonState, Warping};
use crate::debug::DebugMode;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::{DrawIntersection, DrawLane, DrawRoad};
use crate::sandbox::{GameplayMode, SandboxMode};
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, RewriteColor, ScreenRectangle, TextExt, VerticalAlignment, Widget,
    WrappedWizard,
};
use geom::{Polygon, Speed};
use map_model::{
    connectivity, EditCmd, EditIntersection, IntersectionID, LaneID, LaneType, MapEdits,
    PathConstraints, PermanentMapEdits,
};
use sim::DontDrawAgents;
use std::collections::BTreeSet;

pub struct EditMode {
    tool_panel: WrappedComposite,
    composite: Composite,

    // Retained state from the SandboxMode that spawned us
    mode: GameplayMode,

    // edits name, number of commands
    top_panel_key: (String, usize),
    once: bool,
}

impl EditMode {
    pub fn new(ctx: &mut EventCtx, app: &mut App, mode: GameplayMode) -> EditMode {
        assert!(app.suspended_sim.is_none());
        app.suspended_sim = Some(app.primary.clear_sim());
        let edits = app.primary.map.get_edits();
        EditMode {
            tool_panel: tool_panel(ctx, app),
            composite: make_topcenter(ctx, app),
            mode,
            top_panel_key: (edits.edits_name.clone(), edits.commands.len()),
            once: true,
        }
    }

    fn quit(&self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        assert!(app.suspended_sim.is_some());
        app.suspended_sim = None;

        ctx.loading_screen("apply edits", |ctx, mut timer| {
            app.layer = None;
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
            // Parking state might've changed
            app.primary.clear_sim();
            // Autosave
            if app.primary.map.get_edits().edits_name != "untitled edits" {
                app.primary.map.save_edits();
            }
            Transition::PopThenReplace(Box::new(SandboxMode::new(ctx, app, self.mode.clone())))
        })
    }
}

impl State for EditMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // Can't do this in the constructor, because SandboxMode's on_destroy clears out the layer
        if self.once {
            // Once is never...
            self.once = false;
            // apply_map_edits will do the job later
            app.layer = Some(Box::new(crate::layer::map::Static::edits(ctx, app)));
        }
        {
            let edits = app.primary.map.get_edits();
            let top_panel_key = (edits.edits_name.clone(), edits.commands.len());
            if self.top_panel_key != top_panel_key {
                self.top_panel_key = top_panel_key;
                self.composite = make_topcenter(ctx, app);
            }
        }

        ctx.canvas_movement();
        // Restrict what can be selected.
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            );
            if let Some(ID::Lane(l)) = app.primary.current_selection {
                if !can_edit_lane(&self.mode, l, app) {
                    app.primary.current_selection = None;
                }
            } else if let Some(ID::Intersection(_)) = app.primary.current_selection {
            } else if let Some(ID::Road(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }

        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx, app)));
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "load edits" => {
                    // Autosave first
                    if app.primary.map.get_edits().edits_name != "untitled edits" {
                        app.primary.map.save_edits();
                    }
                    return Transition::Push(make_load_edits(
                        app,
                        self.composite.rect_of("load edits").clone(),
                        self.mode.clone(),
                    ));
                }
                "bulk edit" => {
                    return Transition::Push(bulk::PaintSelect::new(ctx, app));
                }
                "finish editing" => {
                    return self.quit(ctx, app);
                }
                "save edits as" => {
                    return Transition::Push(WizardState::new(Box::new(|wiz, ctx, app| {
                        save_edits_as(&mut wiz.wrap(ctx), app)?;
                        Some(Transition::Pop)
                    })));
                }
                "reset edits" => {
                    if app.primary.map.get_edits().edits_name != "untitled edits" {
                        // Autosave, then cut over to blank edits.
                        app.primary.map.save_edits();
                    }
                    apply_map_edits(ctx, app, MapEdits::new());
                }
                "undo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    let id = match edits.commands.pop().unwrap() {
                        EditCmd::ChangeLaneType { id, .. } => ID::Lane(id),
                        EditCmd::ReverseLane { l, .. } => ID::Lane(l),
                        EditCmd::ChangeSpeedLimit { id, .. } => ID::Road(id),
                        EditCmd::ChangeIntersection { i, .. } => ID::Intersection(i),
                    };
                    apply_map_edits(ctx, app, edits);
                    return Transition::Push(Warping::new(
                        ctx,
                        id.canonical_point(&app.primary).unwrap(),
                        None,
                        Some(id),
                        &mut app.primary,
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            if let Some(id) = &app.primary.current_selection {
                if app.per_obj.left_click(ctx, "edit this") {
                    return Transition::Push(Warping::new(
                        ctx,
                        id.canonical_point(&app.primary).unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    ));
                }
            }
        } else {
            if let Some(ID::Intersection(id)) = app.primary.current_selection {
                if app.primary.map.maybe_get_stop_sign(id).is_some()
                    && self.mode.can_edit_stop_signs()
                    && app.per_obj.left_click(ctx, "edit stop signs")
                {
                    return Transition::Push(Box::new(StopSignEditor::new(id, ctx, app)));
                }
                if app.primary.map.maybe_get_traffic_signal(id).is_some()
                    && app.per_obj.left_click(ctx, "edit traffic signal")
                {
                    return Transition::Push(Box::new(TrafficSignalEditor::new(id, ctx, app)));
                }
                if app.primary.map.get_i(id).is_closed()
                    && app.per_obj.left_click(ctx, "re-open closed intersection")
                {
                    // This resets to the original state; it doesn't undo the closure to the last
                    // state. Seems reasonable to me.
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeIntersection {
                        i: id,
                        old: app.primary.map.get_i_edit(id),
                        new: edits.original_intersections[&id].clone(),
                    });
                    apply_map_edits(ctx, app, edits);
                }
            }
            if let Some(ID::Lane(l)) = app.primary.current_selection {
                if app.per_obj.left_click(ctx, "edit lane") {
                    return Transition::Push(Box::new(LaneEditor::new(
                        ctx,
                        app,
                        l,
                        self.mode.clone(),
                    )));
                }
            }
        }

        match self.tool_panel.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "back" => self.quit(ctx, app),
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // TODO Maybe this should be part of app.draw
        // TODO This has an X button, but we never call update and allow it to be changed. Should
        // just omit the button.
        if let Some(ref l) = app.layer {
            l.draw(g, app);
        }

        self.tool_panel.draw(g);
        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);
    }
}

pub fn save_edits_as(wizard: &mut WrappedWizard, app: &mut App) -> Option<()> {
    let map = &mut app.primary.map;
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
                let l = l.trim().to_string();
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

fn make_load_edits(app: &App, btn: ScreenRectangle, mode: GameplayMode) -> Box<dyn State> {
    let current_edits_name = app.primary.map.get_edits().edits_name.clone();

    // TODO Weird behavior: if we cancel out of this, the current edits remain blanked out. Woops?

    WizardState::new(Box::new(move |wiz, ctx, app| {
        let mut wizard = wiz.wrap(ctx);

        if app.primary.map.unsaved_edits() {
            let save = "save edits";
            let discard = "discard";
            if wizard
                .choose_string("Save current edits first?", || vec![save, discard])?
                .as_str()
                == save
            {
                save_edits_as(&mut wizard, app)?;
                wizard.reset();
            }
        }

        // We need to clear out the current edits first, or from_permanent won't work.
        apply_map_edits(wizard.ctx, app, MapEdits::new());

        let (_, new_edits) = wizard.choose_exact(
            (
                HorizontalAlignment::Centered(btn.center().x),
                VerticalAlignment::Below(btn.y2 + 15.0),
            ),
            None,
            || {
                let mut list = Choice::from(
                    abstutil::load_all_objects(abstutil::path_all_edits(
                        app.primary.map.get_name(),
                    ))
                    .into_iter()
                    .filter_map(|(path, perma)| {
                        PermanentMapEdits::from_permanent(perma, &app.primary.map)
                            .map(|edits| (path, edits))
                            .ok()
                    })
                    .filter(|(_, edits)| {
                        mode.allows(edits) && edits.edits_name != current_edits_name
                    })
                    .collect(),
                );
                list.push(Choice::new("start over with blank edits", MapEdits::new()));
                list
            },
        )?;
        apply_map_edits(ctx, app, new_edits);
        Some(Transition::Pop)
    }))
}

fn make_topcenter(ctx: &mut EventCtx, app: &App) -> Composite {
    // TODO Support redo. Bit harder here to reset the redo_stack when the edits
    // change, because nested other places modify it too.
    Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Line("Editing map").small_heading().draw(ctx).margin(5),
                Widget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 30.0))]),
                )
                .margin(5),
                Btn::text_fg(format!("{} ▼", &app.primary.map.get_edits().edits_name))
                    .build(ctx, "load edits", lctrl(Key::L))
                    .margin(5),
                Btn::svg_def("../data/system/assets/tools/save.svg")
                    .build(ctx, "save edits as", lctrl(Key::S))
                    .margin(5),
                (if !app.primary.map.get_edits().commands.is_empty() {
                    Btn::svg_def("../data/system/assets/tools/undo.svg").build(
                        ctx,
                        "undo",
                        lctrl(Key::Z),
                    )
                } else {
                    Widget::draw_svg_transform(
                        ctx,
                        "../data/system/assets/tools/undo.svg",
                        RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                    )
                })
                .margin(15),
            ])
            .centered(),
            Widget::row(vec![
                if !app.primary.map.get_edits().commands.is_empty() {
                    Btn::text_fg("reset edits").build_def(ctx, None)
                } else {
                    Btn::text_fg("reset edits").inactive(ctx)
                }
                .margin_right(15),
                Btn::text_fg("bulk edit")
                    .build_def(ctx, hotkey(Key::B))
                    .margin_right(15),
                Btn::text_bg1("finish editing").build_def(ctx, hotkey(Key::Escape)),
            ]),
        ])
        .padding(5)
        .bg(app.cs.panel_bg),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

pub fn apply_map_edits(ctx: &mut EventCtx, app: &mut App, edits: MapEdits) {
    let mut timer = Timer::new("apply map edits");

    let (roads_changed, turns_deleted, turns_added, mut modified_intersections) =
        app.primary.map.apply_edits(edits, &mut timer);

    for r in roads_changed {
        let road = app.primary.map.get_r(r);
        app.primary.draw_map.roads[r.0] =
            DrawRoad::new(road, &app.primary.map, &app.cs, ctx.prerender);

        // An edit to one lane potentially affects markings in all lanes in the same road, because
        // of one-way markings, driving lines, etc.
        for l in road.all_lanes() {
            let lane = app.primary.map.get_l(l);
            app.primary.draw_map.lanes[l.0] = DrawLane::new(
                lane,
                &app.primary.map,
                app.primary.current_flags.draw_lane_markings,
                &app.cs,
                &mut timer,
            )
            .finish(ctx.prerender, &app.cs, lane);
        }
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
        app.primary.draw_map.intersections[i.0] = DrawIntersection::new(
            app.primary.map.get_i(i),
            &app.primary.map,
            &app.cs,
            ctx.prerender,
            &mut timer,
        );
    }

    if app.layer.as_ref().and_then(|l| l.name()) == Some("map edits") {
        app.layer = Some(Box::new(crate::layer::map::Static::edits(ctx, app)));
    }
}

pub fn can_edit_lane(mode: &GameplayMode, l: LaneID, app: &App) -> bool {
    mode.can_edit_lanes()
        && !app.primary.map.get_l(l).is_sidewalk()
        && app.primary.map.get_l(l).lane_type != LaneType::SharedLeftTurn
}

pub fn close_intersection(
    ctx: &mut EventCtx,
    app: &mut App,
    i: IntersectionID,
    pop_once: bool,
) -> Transition {
    let mut edits = app.primary.map.get_edits().clone();
    edits.commands.push(EditCmd::ChangeIntersection {
        i,
        old: app.primary.map.get_i_edit(i),
        new: EditIntersection::Closed,
    });
    apply_map_edits(ctx, app, edits);

    let (_, disconnected) = connectivity::find_scc(&app.primary.map, PathConstraints::Pedestrian);
    if disconnected.is_empty() {
        // Success! Quit the stop sign / signal editor.
        if pop_once {
            return Transition::Pop;
        } else {
            return Transition::PopTwice;
        }
    }

    let mut edits = app.primary.map.get_edits().clone();
    edits.commands.pop();
    apply_map_edits(ctx, app, edits);

    let mut err_state = msg(
        "Error",
        vec![format!(
            "Can't close this intersection; {} sidewalks disconnected",
            disconnected.len()
        )],
    );

    let color = Color::RED;
    let mut c = Colorer::discrete(ctx, "", Vec::new(), vec![("disconnected", color)]);
    for l in disconnected {
        c.add_l(l, color, &app.primary.map);
    }

    err_state.downcast_mut::<WizardState>().unwrap().also_draw = Some(c.build_zoomed(ctx, app));
    if pop_once {
        Transition::Push(err_state)
    } else {
        Transition::Replace(err_state)
    }
}

#[allow(unused)]
pub fn check_parking_blackholes(
    ctx: &mut EventCtx,
    app: &mut App,
    edits: MapEdits,
) -> Option<Box<dyn State>> {
    let orig_edits = app.primary.map.get_edits().clone();
    let mut ok_originally = BTreeSet::new();
    for l in app.primary.map.all_lanes() {
        if l.parking_blackhole.is_none() {
            ok_originally.insert(l.id);
            // TODO Only matters if there's any parking here anyways
        }
    }

    apply_map_edits(ctx, app, edits);
    let color = Color::RED;
    let mut num_problems = 0;
    let mut c = Colorer::discrete(ctx, "", Vec::new(), vec![("parking disconnected", color)]);
    for (l, _) in
        connectivity::redirect_parking_blackholes(&app.primary.map, &mut Timer::throwaway())
    {
        if ok_originally.contains(&l) {
            num_problems += 1;
            c.add_l(l, color, &app.primary.map);
        }
    }
    if num_problems == 0 {
        None
    } else {
        apply_map_edits(ctx, app, orig_edits);
        let mut err_state = msg(
            "Error",
            vec![format!("{} lanes have parking disconnected", num_problems)],
        );
        err_state.downcast_mut::<WizardState>().unwrap().also_draw = Some(c.build_zoomed(ctx, app));
        Some(err_state)
    }
}

pub fn change_speed_limit(ctx: &mut EventCtx, default: Speed) -> Widget {
    Widget::row(vec![
        "Change speed limit:"
            .draw_text(ctx)
            .centered_vert()
            .margin_right(5),
        Widget::dropdown(
            ctx,
            "speed limit",
            default,
            vec![
                Choice::new("10 mph", Speed::miles_per_hour(10.0)),
                Choice::new("15 mph", Speed::miles_per_hour(15.0)),
                Choice::new("20 mph", Speed::miles_per_hour(20.0)),
                Choice::new("25 mph", Speed::miles_per_hour(25.0)),
                Choice::new("30 mph", Speed::miles_per_hour(30.0)),
                Choice::new("35 mph", Speed::miles_per_hour(35.0)),
                Choice::new("40 mph", Speed::miles_per_hour(40.0)),
                Choice::new("45 mph", Speed::miles_per_hour(45.0)),
                Choice::new("50 mph", Speed::miles_per_hour(50.0)),
                Choice::new("55 mph", Speed::miles_per_hour(55.0)),
                Choice::new("60 mph", Speed::miles_per_hour(60.0)),
                Choice::new("65 mph", Speed::miles_per_hour(65.0)),
                Choice::new("70 mph", Speed::miles_per_hour(70.0)),
                // Don't need anything higher. Though now I kind of miss 3am drives on TX-71...
            ],
        ),
    ])
}

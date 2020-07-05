mod bulk;
mod cluster_traffic_signals;
mod lanes;
mod stop_signs;
mod traffic_signals;
mod validate;
mod zones;

pub use self::cluster_traffic_signals::ClusterTrafficSignalEditor;
pub use self::lanes::LaneEditor;
pub use self::stop_signs::StopSignEditor;
pub use self::traffic_signals::TrafficSignalEditor;
pub use self::validate::{
    check_parking_blackholes, check_sidewalk_connectivity, try_change_lt, try_reverse,
};
use crate::app::{App, ShowEverything};
use crate::common::{tool_panel, CommonState, Warping};
use crate::debug::DebugMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::{DrawIntersection, DrawRoad};
use crate::sandbox::{GameplayMode, SandboxMode, TimeWarpScreen};
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, PersistentSplit, RewriteColor, Text, TextExt, VerticalAlignment, Widget,
    WrappedWizard,
};
use geom::Speed;
use map_model::{EditCmd, IntersectionID, LaneID, LaneType, MapEdits, PermanentMapEdits};
use sim::DontDrawAgents;
use std::collections::BTreeSet;

pub struct EditMode {
    tool_panel: WrappedComposite,
    top_center: Composite,
    changelist: Composite,
    orig_edits: MapEdits,
    orig_dirty: bool,

    // Retained state from the SandboxMode that spawned us
    mode: GameplayMode,

    // edits name, number of commands
    changelist_key: (String, usize),

    unzoomed: Drawable,
    zoomed: Drawable,
}

impl EditMode {
    pub fn new(ctx: &mut EventCtx, app: &mut App, mode: GameplayMode) -> EditMode {
        let orig_dirty = app.primary.dirty_from_edits;
        assert!(app.suspended_sim.is_none());
        app.suspended_sim = Some(app.primary.clear_sim());
        let edits = app.primary.map.get_edits();
        let layer = crate::layer::map::Static::edits(ctx, app);
        EditMode {
            tool_panel: tool_panel(ctx),
            top_center: make_topcenter(ctx, app, &mode),
            changelist: make_changelist(ctx, app),
            orig_edits: edits.clone(),
            orig_dirty,
            mode,
            changelist_key: (edits.edits_name.clone(), edits.commands.len()),
            unzoomed: layer.unzoomed,
            zoomed: layer.zoomed,
        }
    }

    fn quit(&self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let old_sim = app.suspended_sim.take().unwrap();

        // If nothing changed, short-circuit
        if app.primary.map.get_edits() == &self.orig_edits {
            app.primary.sim = old_sim;
            app.primary.dirty_from_edits = self.orig_dirty;
            // Could happen if we load some edits, then load whatever we entered edit mode with.
            ctx.loading_screen("apply edits", |_, mut timer| {
                app.primary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);
            });
            return Transition::Pop;
        }

        ctx.loading_screen("apply edits", move |ctx, mut timer| {
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
            // Parking state might've changed
            app.primary.clear_sim();
            if app.opts.resume_after_edit {
                if self.mode.reset_after_edits() {
                    Transition::PopThenReplaceThenPush(
                        Box::new(SandboxMode::new(ctx, app, self.mode.clone())),
                        TimeWarpScreen::new(ctx, app, old_sim.time(), false),
                    )
                } else {
                    app.primary.sim = old_sim;
                    app.primary.dirty_from_edits = true;
                    Transition::Pop
                }
            } else {
                Transition::PopThenReplace(Box::new(SandboxMode::new(ctx, app, self.mode.clone())))
            }
        })
    }
}

impl State for EditMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        {
            let edits = app.primary.map.get_edits();
            let changelist_key = (edits.edits_name.clone(), edits.commands.len());
            if self.changelist_key != changelist_key {
                self.changelist_key = changelist_key;
                self.changelist = make_changelist(ctx, app);
                let layer = crate::layer::map::Static::edits(ctx, app);
                self.unzoomed = layer.unzoomed;
                self.zoomed = layer.zoomed;
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
            } else if let Some(ID::Intersection(i)) = app.primary.current_selection {
                if app.primary.map.maybe_get_stop_sign(i).is_some()
                    && !self.mode.can_edit_stop_signs()
                {
                    app.primary.current_selection = None;
                }
            } else if let Some(ID::Road(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }

        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::D).unwrap()) {
            return Transition::Push(Box::new(DebugMode::new(ctx)));
        }

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "bulk edit" => {
                    return Transition::Push(bulk::PaintSelect::new(ctx, app));
                }
                "finish editing" => {
                    return self.quit(ctx, app);
                }
                _ => unreachable!(),
            },
            None => {}
        }
        match self.changelist.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "load edits" => {
                    if app.primary.map.unsaved_edits() {
                        return Transition::PushTwice(
                            make_load_edits(app, self.mode.clone()),
                            SaveEdits::new(
                                ctx,
                                app,
                                "Do you want to save your edits first?",
                                true,
                                Some(Transition::PopTwice),
                            ),
                        );
                    } else {
                        return Transition::Push(make_load_edits(app, self.mode.clone()));
                    }
                }
                "save edits as" | "save edits" => {
                    return Transition::Push(SaveEdits::new(
                        ctx,
                        app,
                        "Save your edits",
                        false,
                        Some(Transition::Pop),
                    ));
                }
                "undo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    let id = cmd_to_id(&edits.commands.pop().unwrap());
                    apply_map_edits(ctx, app, edits);
                    return Transition::Push(Warping::new(
                        ctx,
                        id.canonical_point(&app.primary).unwrap(),
                        Some(10.0),
                        Some(id),
                        &mut app.primary,
                    ));
                }
                x => {
                    let idx = x["most recent change #".len()..].parse::<usize>().unwrap();
                    let id = cmd_to_id(
                        &app.primary.map.get_edits().commands
                            [app.primary.map.get_edits().commands.len() - idx],
                    );
                    return Transition::Push(Warping::new(
                        ctx,
                        id.canonical_point(&app.primary).unwrap(),
                        Some(10.0),
                        Some(id),
                        &mut app.primary,
                    ));
                }
            },
            None => {}
        }
        // Just kind of constantly scrape this
        app.opts.resume_after_edit = self.top_center.persistent_split_value("finish editing");

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
                if let Some(state) = maybe_edit_intersection(ctx, app, id, &self.mode) {
                    return Transition::Push(state);
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
        self.tool_panel.draw(g);
        self.top_center.draw(g);
        self.changelist.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        CommonState::draw_osd(g, app);
    }
}

pub struct SaveEdits {
    composite: Composite,
    current_name: String,
    cancel: Option<Transition>,
    reset: bool,
}

impl SaveEdits {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        title: &str,
        discard: bool,
        cancel: Option<Transition>,
    ) -> Box<dyn State> {
        let initial_name = if app.primary.map.unsaved_edits() {
            String::new()
        } else {
            format!("copy of {}", app.primary.map.get_edits().edits_name)
        };
        let btn = SaveEdits::btn(ctx, app, &initial_name);
        Box::new(SaveEdits {
            current_name: initial_name.clone(),
            composite: Composite::new(Widget::col(vec![
                Line(title).small_heading().draw(ctx),
                Widget::row(vec![
                    "Name:".draw_text(ctx),
                    Widget::text_entry(ctx, initial_name, true).named("filename"),
                ]),
                Widget::row(vec![
                    btn,
                    if discard {
                        Btn::text_bg2("Discard edits").build_def(ctx, None)
                    } else {
                        Widget::nothing()
                    },
                    if cancel.is_some() {
                        Btn::text_bg2("Cancel").build_def(ctx, hotkey(Key::Escape))
                    } else {
                        Widget::nothing()
                    },
                ]),
            ]))
            .build(ctx),
            cancel,
            reset: discard,
        })
    }

    fn btn(ctx: &mut EventCtx, app: &App, candidate: &str) -> Widget {
        if candidate.is_empty() {
            Btn::text_bg2("Save").inactive(ctx)
        } else if abstutil::file_exists(abstutil::path_edits(app.primary.map.get_name(), candidate))
        {
            Btn::text_bg2("Overwrite existing edits").build_def(ctx, None)
        } else {
            Btn::text_bg2("Save").build_def(ctx, hotkey(Key::Enter))
        }
        .named("save")
    }
}

impl State for SaveEdits {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Save" | "Overwrite existing edits" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.edits_name = self.current_name.clone();
                    app.primary
                        .map
                        .must_apply_edits(edits, &mut Timer::new("name map edits"));
                    app.primary.map.save_edits();
                    if self.reset {
                        apply_map_edits(ctx, app, MapEdits::new());
                    }
                    return Transition::Pop;
                }
                "Discard edits" => {
                    apply_map_edits(ctx, app, MapEdits::new());
                    return Transition::Pop;
                }
                "Cancel" => {
                    return self.cancel.take().unwrap();
                }
                _ => unreachable!(),
            },
            None => {}
        }
        let name = self.composite.text_box("filename");
        if name != self.current_name {
            self.current_name = name;
            let btn = SaveEdits::btn(ctx, app, &self.current_name).margin_right(10);
            self.composite.replace(ctx, "save", btn);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

pub fn save_edits_as(wizard: &mut WrappedWizard, app: &mut App) -> Option<()> {
    let map = &mut app.primary.map;
    let (prompt, new_default_name) = if map.get_edits().edits_name == "untitled edits" {
        ("Name these edits", "".to_string())
    } else {
        (
            "Name the new copy of these edits",
            format!("copy of {}", map.get_edits().edits_name),
        )
    };

    let name = loop {
        let candidate = wizard.input_something(
            prompt,
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
    map.must_apply_edits(edits, &mut Timer::new("name map edits"));
    map.save_edits();
    Some(())
}

fn make_load_edits(app: &App, mode: GameplayMode) -> Box<dyn State> {
    let current_edits_name = app.primary.map.get_edits().edits_name.clone();

    WizardState::new(Box::new(move |wiz, ctx, app| {
        let (_, new_edits) = wiz.wrap(ctx).choose("Load which edits?", || {
            let mut list = Choice::from(
                abstutil::load_all_objects(abstutil::path_all_edits(app.primary.map.get_name()))
                    .into_iter()
                    .chain(abstutil::load_all_objects::<PermanentMapEdits>(
                        "../data/system/proposals".to_string(),
                    ))
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
        })?;
        apply_map_edits(ctx, app, new_edits);
        Some(Transition::Pop)
    }))
}

fn make_topcenter(ctx: &mut EventCtx, app: &App, mode: &GameplayMode) -> Composite {
    Composite::new(Widget::col(vec![
        Line("Editing map")
            .small_heading()
            .draw(ctx)
            .centered_horiz(),
        Widget::row(vec![
            if mode.can_edit_lanes() {
                Btn::text_fg("bulk edit").build_def(ctx, hotkey(Key::B))
            } else {
                Btn::text_fg("bulk edit").inactive(ctx)
            },
            PersistentSplit::new(
                ctx,
                "finish editing",
                app.opts.resume_after_edit,
                hotkey(Key::Escape),
                vec![
                    Choice::new(
                        format!(
                            "Finish & resume from {}",
                            app.suspended_sim.as_ref().unwrap().time().ampm_tostring()
                        ),
                        true,
                    ),
                    Choice::new("Finish & restart from midnight", false),
                ],
            )
            .bg(app.cs.section_bg),
        ]),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

pub fn apply_map_edits(ctx: &mut EventCtx, app: &mut App, edits: MapEdits) {
    let mut timer = Timer::new("apply map edits");

    let (roads_changed, turns_deleted, turns_added, mut modified_intersections) =
        app.primary.map.must_apply_edits(edits, &mut timer);

    for r in roads_changed {
        let road = app.primary.map.get_r(r);
        app.primary.draw_map.roads[r.0] =
            DrawRoad::new(road, &app.primary.map, &app.cs, ctx.prerender);

        // An edit to one lane potentially affects markings in all lanes in the same road, because
        // of one-way markings, driving lines, etc.
        for l in road.all_lanes() {
            app.primary.draw_map.lanes[l.0].clear_rendering();
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

    // Autosave
    if app.primary.map.get_edits().edits_name != "untitled edits" {
        app.primary.map.save_edits();
    }
}

pub fn can_edit_lane(mode: &GameplayMode, l: LaneID, app: &App) -> bool {
    mode.can_edit_lanes()
        && !app.primary.map.get_l(l).is_sidewalk()
        && app.primary.map.get_l(l).lane_type != LaneType::SharedLeftTurn
        && app.primary.map.get_l(l).lane_type != LaneType::LightRail
}

pub fn change_speed_limit(ctx: &mut EventCtx, default: Speed) -> Widget {
    Widget::row(vec![
        "Change speed limit:".draw_text(ctx).centered_vert(),
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

pub fn maybe_edit_intersection(
    ctx: &mut EventCtx,
    app: &mut App,
    id: IntersectionID,
    mode: &GameplayMode,
) -> Option<Box<dyn State>> {
    if app.primary.map.maybe_get_stop_sign(id).is_some()
        && mode.can_edit_stop_signs()
        && app.per_obj.left_click(ctx, "edit stop signs")
    {
        return Some(Box::new(StopSignEditor::new(ctx, app, id, mode.clone())));
    }

    if app.primary.map.maybe_get_traffic_signal(id).is_some()
        && app.per_obj.left_click(ctx, "edit traffic signal")
    {
        return Some(Box::new(TrafficSignalEditor::new(
            ctx,
            app,
            id,
            mode.clone(),
        )));
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

    None
}

fn make_changelist(ctx: &mut EventCtx, app: &App) -> Composite {
    // TODO Support redo. Bit harder here to reset the redo_stack when the edits
    // change, because nested other places modify it too.
    let edits = app.primary.map.get_edits();
    let mut col = vec![
        Widget::row(vec![
            Btn::text_fg(format!("{} ↓", &edits.edits_name)).build(
                ctx,
                "load edits",
                lctrl(Key::L),
            ),
            (if edits.commands.is_empty() {
                Widget::draw_svg_transform(
                    ctx,
                    "../data/system/assets/tools/save.svg",
                    RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                )
            } else {
                Btn::svg_def("../data/system/assets/tools/save.svg").build(
                    ctx,
                    "save edits as",
                    lctrl(Key::S),
                )
            })
            .centered_vert(),
            (if !edits.commands.is_empty() {
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
            .centered_vert(),
        ]),
        if app.primary.map.unsaved_edits() {
            Btn::text_fg("Unsaved edits").build(ctx, "save edits", None)
        } else {
            Btn::text_fg("Autosaved!").inactive(ctx)
        },
        Text::from_multiline(vec![
            Line(format!("{} lane types changed", edits.original_lts.len())),
            Line(format!("{} lanes reversed", edits.reversed_lanes.len())),
            Line(format!(
                "{} speed limits changed",
                edits.changed_speed_limits.len()
            )),
            Line(format!(
                "{} intersections changed",
                edits.original_intersections.len()
            )),
        ])
        .draw(ctx),
    ];

    for (idx, cmd) in edits.commands.iter().rev().take(5).enumerate() {
        col.push(
            Btn::plaintext(format!("{}) {}", idx + 1, cmd.short_name())).build(
                ctx,
                format!("most recent change #{}", idx + 1),
                None,
            ),
        );
    }
    if edits.commands.len() > 5 {
        col.push(format!("{} more...", edits.commands.len()).draw_text(ctx));
    }

    Composite::new(Widget::col(col))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx)
}

fn cmd_to_id(cmd: &EditCmd) -> ID {
    match cmd {
        EditCmd::ChangeLaneType { id, .. } => ID::Lane(*id),
        EditCmd::ReverseLane { l, .. } => ID::Lane(*l),
        EditCmd::ChangeSpeedLimit { id, .. } => ID::Road(*id),
        EditCmd::ChangeIntersection { i, .. } => ID::Intersection(*i),
        EditCmd::ChangeAccessRestrictions { id, .. } => ID::Road(*id),
    }
}

mod bulk;
mod cluster_traffic_signals;
mod lanes;
mod routes;
mod select;
mod stop_signs;
mod traffic_signals;
mod validate;
mod zones;

pub use self::cluster_traffic_signals::ClusterTrafficSignalEditor;
pub use self::lanes::LaneEditor;
pub use self::routes::RouteEditor;
pub use self::stop_signs::StopSignEditor;
pub use self::traffic_signals::TrafficSignalEditor;
pub use self::validate::{check_blackholes, check_sidewalk_connectivity, try_change_lt};
use crate::app::{App, ShowEverything};
use crate::common::{tool_panel, CommonState, Warping};
use crate::debug::DebugMode;
use crate::game::{PopupMsg, State, Transition};
use crate::helpers::ID;
use crate::options::OptionsPanel;
use crate::render::DrawMap;
use crate::sandbox::GameplayMode;
use abstutil::Timer;
use geom::Speed;
use map_model::{EditCmd, IntersectionID, LaneID, LaneType, MapEdits};
use maplit::btreeset;
use sim::DontDrawAgents;
use std::collections::BTreeSet;
use widgetry::{
    hotkey, lctrl, Btn, Choice, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Menu, Outcome, Panel, RewriteColor, Text, TextExt, VerticalAlignment, Widget,
};

pub struct EditMode {
    tool_panel: Panel,
    top_center: Panel,
    changelist: Panel,
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
    pub fn new(ctx: &mut EventCtx, app: &mut App, mode: GameplayMode) -> Box<dyn State> {
        let orig_dirty = app.primary.dirty_from_edits;
        assert!(app.suspended_sim.is_none());
        app.suspended_sim = Some(app.primary.clear_sim());
        let edits = app.primary.map.get_edits();
        let layer = crate::layer::map::Static::edits(ctx, app);
        Box::new(EditMode {
            tool_panel: tool_panel(ctx),
            top_center: make_topcenter(ctx, app, &mode),
            changelist: make_changelist(ctx, app),
            orig_edits: edits.clone(),
            orig_dirty,
            mode,
            changelist_key: (edits.edits_name.clone(), edits.commands.len()),
            unzoomed: layer.unzoomed,
            zoomed: layer.zoomed,
        })
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

        ctx.loading_screen("apply edits", move |_, mut timer| {
            app.primary
                .map
                .recalculate_pathfinding_after_edits(&mut timer);
            app.primary.sim = old_sim;
            app.primary.dirty_from_edits = true;
            app.primary
                .sim
                .handle_live_edited_traffic_signals(&app.primary.map);
            app.primary.sim.handle_live_edits(&app.primary.map);
            Transition::Pop
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

        if app.opts.dev && ctx.input.pressed(lctrl(Key::D)) {
            return Transition::Push(Box::new(DebugMode::new(ctx)));
        }

        match self.top_center.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "bulk edit" => {
                    return Transition::Push(bulk::BulkSelect::new(ctx, app));
                }
                "finish editing" => {
                    return self.quit(ctx, app);
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        match self.changelist.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "load edits" => {
                    if app.primary.map.unsaved_edits() {
                        return Transition::Multi(vec![
                            Transition::Push(LoadEdits::new(ctx, app, self.mode.clone())),
                            Transition::Push(SaveEdits::new(
                                ctx,
                                app,
                                "Do you want to save your edits first?",
                                true,
                                Some(Transition::Multi(vec![Transition::Pop, Transition::Pop])),
                            )),
                        ]);
                    } else {
                        return Transition::Push(LoadEdits::new(ctx, app, self.mode.clone()));
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
                    let maybe_id = cmd_to_id(&edits.commands.pop().unwrap());
                    apply_map_edits(ctx, app, edits);
                    if let Some(id) = maybe_id {
                        return Transition::Push(Warping::new(
                            ctx,
                            id.canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            Some(id),
                            &mut app.primary,
                        ));
                    }
                }
                x => {
                    let idx = x["most recent change #".len()..].parse::<usize>().unwrap();
                    if let Some(id) = cmd_to_id(
                        &app.primary.map.get_edits().commands
                            [app.primary.map.get_edits().commands.len() - idx],
                    ) {
                        return Transition::Push(Warping::new(
                            ctx,
                            id.canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            Some(id),
                            &mut app.primary,
                        ));
                    }
                }
            },
            _ => {}
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
                if let Some(state) = maybe_edit_intersection(ctx, app, id, &self.mode) {
                    return Transition::Push(state);
                }
            }
            if let Some(ID::Lane(l)) = app.primary.current_selection {
                if app.per_obj.left_click(ctx, "edit lane") {
                    return Transition::Push(LaneEditor::new(ctx, app, l, self.mode.clone()));
                }
            }
        }

        match self.tool_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back" => self.quit(ctx, app),
                "settings" => Transition::Push(OptionsPanel::new(ctx, app)),
                _ => unreachable!(),
            },
            _ => Transition::Keep,
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
    panel: Panel,
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
            panel: Panel::new(Widget::col(vec![
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
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
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
            _ => {}
        }
        let name = self.panel.text_box("filename");
        if name != self.current_name {
            self.current_name = name;
            let btn = SaveEdits::btn(ctx, app, &self.current_name);
            self.panel.replace(ctx, "save", btn);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct LoadEdits {
    panel: Panel,
    mode: GameplayMode,
}

impl LoadEdits {
    fn new(ctx: &mut EventCtx, app: &App, mode: GameplayMode) -> Box<dyn State> {
        let current_edits_name = &app.primary.map.get_edits().edits_name;
        let your_edits = vec![
            Line("Your edits").small_heading().draw(ctx),
            Menu::new(
                ctx,
                abstutil::list_all_objects(abstutil::path_all_edits(app.primary.map.get_name()))
                    .into_iter()
                    .map(|name| Choice::new(name.clone(), ()).active(&name != current_edits_name))
                    .collect(),
            ),
        ];
        // widgetry can't toggle keyboard focus between two menus, so just use buttons for the less
        // common use case.
        let mut proposals = vec![Line("Community proposals").small_heading().draw(ctx)];
        // Up-front filter out proposals that definitely don't fit the current map
        for name in abstutil::list_all_objects(abstutil::path("system/proposals")) {
            let path = abstutil::path(format!("system/proposals/{}.json", name));
            if MapEdits::load(&app.primary.map, path.clone(), &mut Timer::throwaway()).is_ok() {
                proposals.push(Btn::text_fg(&name).build(ctx, path, None));
            }
        }

        Box::new(LoadEdits {
            mode,
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Load edits").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Btn::text_fg("Start over with blank edits").build_def(ctx, None),
                Widget::row(vec![Widget::col(your_edits), Widget::col(proposals)]).evenly_spaced(),
            ]))
            .exact_size_percent(50, 50)
            .build(ctx),
        })
    }
}

impl State for LoadEdits {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                match x.as_ref() {
                    "close" => Transition::Pop,
                    "Start over with blank edits" => {
                        apply_map_edits(ctx, app, MapEdits::new());
                        Transition::Pop
                    }
                    path => {
                        // TODO Kind of a hack. If it ends with .json, it's already a path.
                        // Otherwise it's a result from the menu.
                        let path = if path.ends_with(".json") {
                            path.to_string()
                        } else {
                            abstutil::path_edits(app.primary.map.get_name(), path)
                        };

                        match MapEdits::load(
                            &app.primary.map,
                            path.clone(),
                            &mut Timer::throwaway(),
                        )
                        .and_then(|edits| {
                            if self.mode.allows(&edits) {
                                Ok(edits)
                            } else {
                                Err(
                                    "The current gameplay mode restricts edits. These edits have \
                                     a banned command."
                                        .to_string(),
                                )
                            }
                        }) {
                            Ok(edits) => {
                                apply_map_edits(ctx, app, edits);
                                Transition::Pop
                            }
                            // TODO Hack. Have to replace ourselves, because the Menu might be
                            // invalidated now that something was chosen.
                            Err(err) => {
                                println!("Can't load {}: {}", path, err);
                                Transition::Multi(vec![
                                    Transition::Replace(LoadEdits::new(
                                        ctx,
                                        app,
                                        self.mode.clone(),
                                    )),
                                    // TODO Menu draws at a weird Z-order to deal with tooltips, so
                                    // now the menu underneath
                                    // bleeds through
                                    Transition::Push(PopupMsg::new(
                                        ctx,
                                        "Error",
                                        vec![format!("Can't load {}", path), err.clone()],
                                    )),
                                ])
                            }
                        }
                    }
                }
            }
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.panel.draw(g);
    }
}

fn make_topcenter(ctx: &mut EventCtx, app: &App, mode: &GameplayMode) -> Panel {
    Panel::new(Widget::col(vec![
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
            Btn::text_bg2(format!(
                "Finish & resume from {}",
                app.suspended_sim.as_ref().unwrap().time().ampm_tostring()
            ))
            .build(ctx, "finish editing", hotkey(Key::Escape)),
        ]),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

pub fn apply_map_edits(ctx: &mut EventCtx, app: &mut App, edits: MapEdits) {
    let mut timer = Timer::new("apply map edits");

    let (roads_changed, turns_deleted, turns_added, mut modified_intersections) =
        app.primary.map.must_apply_edits(edits, &mut timer);

    if !roads_changed.is_empty() || !modified_intersections.is_empty() {
        app.primary
            .draw_map
            .draw_all_unzoomed_roads_and_intersections =
            DrawMap::regenerate_unzoomed_layer(&app.primary.map, &app.cs, ctx, &mut timer);
    }

    for r in roads_changed {
        let road = app.primary.map.get_r(r);
        app.primary.draw_map.roads[r.0].clear_rendering();

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
        app.primary.draw_map.intersections[i.0].clear_rendering();
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
        && !app.primary.map.get_l(l).is_walkable()
        && app.primary.map.get_l(l).lane_type != LaneType::SharedLeftTurn
        && !app.primary.map.get_l(l).is_light_rail()
}

pub fn change_speed_limit(ctx: &mut EventCtx, default: Speed) -> Widget {
    let mut choices = vec![
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
    ];
    if !choices.iter().any(|c| c.data == default) {
        choices.push(Choice::new(default.to_string(), default));
    }

    Widget::row(vec![
        "Change speed limit:".draw_text(ctx).centered_vert(),
        Widget::dropdown(ctx, "speed limit", default, choices),
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
        return Some(StopSignEditor::new(ctx, app, id, mode.clone()));
    }

    if app.primary.map.maybe_get_traffic_signal(id).is_some()
        && app.per_obj.left_click(ctx, "edit traffic signal")
    {
        return Some(TrafficSignalEditor::new(
            ctx,
            app,
            btreeset! {id},
            mode.clone(),
        ));
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

fn make_changelist(ctx: &mut EventCtx, app: &App) -> Panel {
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
                    "system/assets/tools/save.svg",
                    RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                )
            } else {
                Btn::svg_def("system/assets/tools/save.svg").build(
                    ctx,
                    "save edits as",
                    lctrl(Key::S),
                )
            })
            .centered_vert(),
            (if !edits.commands.is_empty() {
                Btn::svg_def("system/assets/tools/undo.svg").build(ctx, "undo", lctrl(Key::Z))
            } else {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/undo.svg",
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
            Line(format!("{} roads changed", edits.changed_roads.len())),
            Line(format!(
                "{} intersections changed",
                edits.original_intersections.len()
            )),
        ])
        .draw(ctx),
    ];

    for (idx, cmd) in edits.commands.iter().rev().take(5).enumerate() {
        col.push(
            Btn::plaintext(format!("{}) {}", idx + 1, cmd.short_name(&app.primary.map))).build(
                ctx,
                format!("most recent change #{}", idx + 1),
                None,
            ),
        );
    }
    if edits.commands.len() > 5 {
        col.push(format!("{} more...", edits.commands.len()).draw_text(ctx));
    }

    Panel::new(Widget::col(col))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx)
}

// TODO Ideally a Tab.
fn cmd_to_id(cmd: &EditCmd) -> Option<ID> {
    match cmd {
        EditCmd::ChangeRoad { r, .. } => Some(ID::Road(*r)),
        EditCmd::ChangeIntersection { i, .. } => Some(ID::Intersection(*i)),
        EditCmd::ChangeRouteSchedule { .. } => None,
    }
}

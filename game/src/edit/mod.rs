use maplit::btreeset;

use abstutil::{prettyprint_usize, Timer};
use geom::Speed;
use map_gui::options::OptionsPanel;
use map_gui::render::DrawMap;
use map_gui::tools::{grey_out_map, ChooseSomething, ColorLegend, PopupMsg};
use map_gui::ID;
use map_model::{EditCmd, IntersectionID, LaneID, MapEdits};
use widgetry::{
    lctrl, Choice, Color, ControlState, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Image,
    Key, Line, Menu, Outcome, Panel, State, Text, TextBox, TextExt, VerticalAlignment, Widget,
};

pub use self::roads::RoadEditor;
pub use self::routes::RouteEditor;
pub use self::stop_signs::StopSignEditor;
pub use self::traffic_signals::TrafficSignalEditor;
pub use self::validate::{check_blackholes, check_sidewalk_connectivity};
use crate::app::{App, Transition};
use crate::common::{tool_panel, CommonState, Warping};
use crate::debug::DebugMode;
use crate::sandbox::{GameplayMode, SandboxMode, TimeWarpScreen};

mod heuristics;
mod multiple_roads;
mod roads;
mod routes;
mod stop_signs;
mod traffic_signals;
mod validate;
mod zones;

pub struct EditMode {
    tool_panel: Panel,
    top_center: Panel,
    changelist: Panel,
    orig_edits: MapEdits,
    orig_dirty: bool,

    // Retained state from the SandboxMode that spawned us
    mode: GameplayMode,

    map_edit_key: usize,

    unzoomed: Drawable,
    zoomed: Drawable,
}

impl EditMode {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, mode: GameplayMode) -> Box<dyn State<App>> {
        let orig_dirty = app.primary.dirty_from_edits;
        assert!(app.primary.suspended_sim.is_none());
        app.primary.suspended_sim = Some(app.primary.clear_sim());
        let layer = crate::layer::map::Static::edits(ctx, app);
        Box::new(EditMode {
            tool_panel: tool_panel(ctx),
            top_center: make_topcenter(ctx, app),
            changelist: make_changelist(ctx, app),
            orig_edits: app.primary.map.get_edits().clone(),
            orig_dirty,
            mode,
            map_edit_key: app.primary.map.get_edits_change_key(),
            unzoomed: layer.unzoomed,
            zoomed: layer.zoomed,
        })
    }

    fn quit(&self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let old_sim = app.primary.suspended_sim.take().unwrap();

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
            if GameplayMode::FixTrafficSignals == self.mode {
                app.primary.sim = old_sim;
                app.primary.dirty_from_edits = true;
                app.primary
                    .sim
                    .handle_live_edited_traffic_signals(&app.primary.map);
                Transition::Pop
            } else if app.primary.current_flags.live_map_edits {
                app.primary.sim = old_sim;
                app.primary.dirty_from_edits = true;
                app.primary
                    .sim
                    .handle_live_edited_traffic_signals(&app.primary.map);
                let (trips, parked_cars) = app
                    .primary
                    .sim
                    .handle_live_edits(&app.primary.map, &mut timer);
                if trips == 0 && parked_cars == 0 {
                    Transition::Pop
                } else {
                    Transition::Replace(PopupMsg::new_state(
                        ctx,
                        "Map changes complete",
                        vec![
                            format!(
                                "Your edits interrupted {} trips and displaced {} parked cars",
                                prettyprint_usize(trips),
                                prettyprint_usize(parked_cars)
                            ),
                            "Simulation results won't be finalized unless you restart from \
                             midnight with your changes"
                                .to_string(),
                        ],
                    ))
                }
            } else {
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::Replace(SandboxMode::async_new(
                        app,
                        self.mode.clone(),
                        Box::new(move |ctx, app| {
                            vec![Transition::Push(TimeWarpScreen::new_state(
                                ctx,
                                app,
                                old_sim.time(),
                                None,
                            ))]
                        }),
                    )),
                ])
            }
        })
    }
}

impl State<App> for EditMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        {
            // We would normally use Cached, but so many values depend on one key, so this is more
            // clear.
            let key = app.primary.map.get_edits_change_key();
            if self.map_edit_key != key {
                self.map_edit_key = key;
                self.changelist = make_changelist(ctx, app);
                let layer = crate::layer::map::Static::edits(ctx, app);
                self.unzoomed = layer.unzoomed;
                self.zoomed = layer.zoomed;
            }
        }

        if let Some(t) = CommonState::debug_actions(ctx, app) {
            return t;
        }

        ctx.canvas_movement();
        // Restrict what can be selected.
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_unzoomed_roads_and_intersections(ctx);
            if match app.primary.current_selection {
                Some(ID::Lane(l)) => !self.mode.can_edit_roads() || !can_edit_lane(app, l),
                Some(ID::Intersection(i)) => {
                    !self.mode.can_edit_stop_signs()
                        && app.primary.map.maybe_get_stop_sign(i).is_some()
                }
                Some(ID::Road(_)) => false,
                _ => true,
            } {
                app.primary.current_selection = None;
            }
        }

        if app.opts.dev && ctx.input.pressed(lctrl(Key::D)) {
            return Transition::Push(DebugMode::new_state(ctx, app));
        }

        if let Outcome::Clicked(x) = self.top_center.event(ctx) {
            match x.as_ref() {
                "finish editing" => {
                    return self.quit(ctx, app);
                }
                _ => unreachable!(),
            }
        }
        if let Outcome::Clicked(x) = self.changelist.event(ctx) {
            match x.as_ref() {
                "manage proposals" => {
                    let mode = self.mode.clone();
                    return Transition::Push(ChooseSomething::new_state(
                        ctx,
                        "Manage proposals",
                        vec![
                            Choice::string("rename current proposal"),
                            Choice::string("open a saved proposal").multikey(lctrl(Key::L)),
                            Choice::string("create a blank proposal"),
                            Choice::string("save this proposal as..."),
                            Choice::string("delete this proposal and remove all edits")
                                .fg(ctx.style().text_destructive_color),
                        ],
                        Box::new(move |choice, ctx, app| match choice.as_ref() {
                            "rename current proposal" => {
                                let old_name = app.primary.map.get_edits().edits_name.clone();
                                Transition::Replace(SaveEdits::new_state(
                                    ctx,
                                    app,
                                    format!("Rename \"{}\"", old_name),
                                    false,
                                    Some(Transition::Pop),
                                    Box::new(move |_, app| {
                                        abstio::delete_file(abstio::path_edits(
                                            app.primary.map.get_name(),
                                            &old_name,
                                        ));
                                    }),
                                ))
                            }
                            "open a saved proposal" => {
                                if app.primary.map.unsaved_edits() {
                                    Transition::Multi(vec![
                                        Transition::Replace(LoadEdits::new_state(ctx, app, mode)),
                                        Transition::Push(SaveEdits::new_state(
                                            ctx,
                                            app,
                                            "Do you want to save your proposal first?",
                                            true,
                                            Some(Transition::Multi(vec![
                                                Transition::Pop,
                                                Transition::Pop,
                                            ])),
                                            Box::new(|_, _| {}),
                                        )),
                                    ])
                                } else {
                                    Transition::Replace(LoadEdits::new_state(ctx, app, mode))
                                }
                            }
                            "create a blank proposal" => {
                                if app.primary.map.unsaved_edits() {
                                    Transition::Replace(SaveEdits::new_state(
                                        ctx,
                                        app,
                                        "Do you want to save your proposal first?",
                                        true,
                                        Some(Transition::Pop),
                                        Box::new(|ctx, app| {
                                            apply_map_edits(ctx, app, app.primary.map.new_edits());
                                        }),
                                    ))
                                } else {
                                    apply_map_edits(ctx, app, app.primary.map.new_edits());
                                    Transition::Pop
                                }
                            }
                            "save this proposal as..." => {
                                Transition::Replace(SaveEdits::new_state(
                                    ctx,
                                    app,
                                    format!(
                                        "Save \"{}\" as",
                                        app.primary.map.get_edits().edits_name
                                    ),
                                    false,
                                    Some(Transition::Pop),
                                    Box::new(|_, _| {}),
                                ))
                            }
                            "delete this proposal and remove all edits" => {
                                abstio::delete_file(abstio::path_edits(
                                    app.primary.map.get_name(),
                                    &app.primary.map.get_edits().edits_name,
                                ));
                                apply_map_edits(ctx, app, app.primary.map.new_edits());
                                Transition::Pop
                            }
                            _ => unreachable!(),
                        }),
                    ));
                }
                "load proposal" => {}
                "undo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    let maybe_id = cmd_to_id(&edits.commands.pop().unwrap());
                    apply_map_edits(ctx, app, edits);
                    if let Some(id) = maybe_id {
                        return Transition::Push(Warping::new_state(
                            ctx,
                            app.primary.canonical_point(id.clone()).unwrap(),
                            Some(10.0),
                            Some(id),
                            &mut app.primary,
                        ));
                    }
                }
                x => {
                    let idx = x["change #".len()..].parse::<usize>().unwrap();
                    if let Some(id) = cmd_to_id(&app.primary.map.get_edits().commands[idx - 1]) {
                        return Transition::Push(Warping::new_state(
                            ctx,
                            app.primary.canonical_point(id.clone()).unwrap(),
                            Some(10.0),
                            Some(id),
                            &mut app.primary,
                        ));
                    }
                }
            }
        }

        // So useful that the hotkey should work even before opening the menu
        if ctx.input.pressed(lctrl(Key::L)) {
            if app.primary.map.unsaved_edits() {
                return Transition::Multi(vec![
                    Transition::Push(LoadEdits::new_state(ctx, app, self.mode.clone())),
                    Transition::Push(SaveEdits::new_state(
                        ctx,
                        app,
                        "Do you want to save your proposal first?",
                        true,
                        Some(Transition::Multi(vec![Transition::Pop, Transition::Pop])),
                        Box::new(|_, _| {}),
                    )),
                ]);
            } else {
                return Transition::Push(LoadEdits::new_state(ctx, app, self.mode.clone()));
            }
        }

        if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            if let Some(id) = app.primary.current_selection.clone() {
                if app.per_obj.left_click(ctx, "edit this") {
                    return Transition::Push(Warping::new_state(
                        ctx,
                        app.primary.canonical_point(id).unwrap(),
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
                    return Transition::Push(RoadEditor::new_state(ctx, app, l));
                }
            }
        }

        match self.tool_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "back" => self.quit(ctx, app),
                "settings" => Transition::Push(OptionsPanel::new_state(ctx, app)),
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
    on_success: Box<dyn Fn(&mut EventCtx, &mut App)>,
    reset: bool,
}

impl SaveEdits {
    pub fn new_state<I: Into<String>>(
        ctx: &mut EventCtx,
        app: &App,
        title: I,
        discard: bool,
        cancel: Option<Transition>,
        on_success: Box<dyn Fn(&mut EventCtx, &mut App)>,
    ) -> Box<dyn State<App>> {
        let initial_name = if app.primary.map.unsaved_edits() {
            String::new()
        } else {
            format!("copy of {}", app.primary.map.get_edits().edits_name)
        };
        let mut save = SaveEdits {
            current_name: initial_name.clone(),
            panel: Panel::new_builder(Widget::col(vec![
                Line(title).small_heading().into_widget(ctx),
                Widget::row(vec![
                    "Name:".text_widget(ctx).centered_vert(),
                    TextBox::default_widget(ctx, "filename", initial_name),
                ]),
                // TODO Want this to always consistently be one line high, but it isn't for a blank
                // line
                Text::new().into_widget(ctx).named("warning"),
                Widget::row(vec![
                    if discard {
                        ctx.style()
                            .btn_solid_destructive
                            .text("Discard proposal")
                            .build_def(ctx)
                    } else {
                        Widget::nothing()
                    },
                    if cancel.is_some() {
                        ctx.style()
                            .btn_outline
                            .text("Cancel")
                            .hotkey(Key::Escape)
                            .build_def(ctx)
                    } else {
                        Widget::nothing()
                    },
                    ctx.style()
                        .btn_solid_primary
                        .text("Save")
                        .disabled(true)
                        .build_def(ctx),
                ])
                .align_right(),
            ]))
            .build(ctx),
            cancel,
            on_success,
            reset: discard,
        };
        save.recalc_btn(ctx, app);
        Box::new(save)
    }

    fn recalc_btn(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.current_name.is_empty() {
            self.panel.replace(
                ctx,
                "Save",
                ctx.style()
                    .btn_solid_primary
                    .text("Save")
                    .disabled(true)
                    .build_def(ctx),
            );
            self.panel
                .replace(ctx, "warning", Text::new().into_widget(ctx));
        } else if abstio::file_exists(abstio::path_edits(
            app.primary.map.get_name(),
            &self.current_name,
        )) {
            self.panel.replace(
                ctx,
                "Save",
                ctx.style()
                    .btn_solid_primary
                    .text("Save")
                    .disabled(true)
                    .build_def(ctx),
            );
            self.panel.replace(
                ctx,
                "warning",
                Line("A proposal with this name already exists")
                    .fg(Color::hex("#FF5E5E"))
                    .into_widget(ctx),
            );
        } else {
            self.panel.replace(
                ctx,
                "Save",
                ctx.style()
                    .btn_solid_primary
                    .text("Save")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
            );
            self.panel
                .replace(ctx, "warning", Text::new().into_widget(ctx));
        }
    }
}

impl State<App> for SaveEdits {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Save" | "Overwrite existing proposal" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.edits_name = self.current_name.clone();
                    app.primary.map.must_apply_edits(edits);
                    app.primary.map.save_edits();
                    if self.reset {
                        apply_map_edits(ctx, app, app.primary.map.new_edits());
                    }
                    (self.on_success)(ctx, app);
                    return Transition::Pop;
                }
                "Discard proposal" => {
                    apply_map_edits(ctx, app, app.primary.map.new_edits());
                    return Transition::Pop;
                }
                "Cancel" => {
                    return self.cancel.take().unwrap();
                }
                _ => unreachable!(),
            }
        }
        let name = self.panel.text_box("filename");
        if name != self.current_name {
            self.current_name = name;
            self.recalc_btn(ctx, app);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

pub struct LoadEdits {
    panel: Panel,
    mode: GameplayMode,
}

impl LoadEdits {
    /// Mode is just used for `allows`.
    pub fn new_state(ctx: &mut EventCtx, app: &App, mode: GameplayMode) -> Box<dyn State<App>> {
        let current_edits_name = &app.primary.map.get_edits().edits_name;

        let mut your_proposals =
            abstio::list_all_objects(abstio::path_all_edits(app.primary.map.get_name()))
                .into_iter()
                .map(|name| Choice::new(name.clone(), ()).active(&name != current_edits_name))
                .collect::<Vec<_>>();
        // These're sorted alphabetically, but the "Untitled Proposal"s wind up before lowercase
        // names!
        your_proposals.sort_by_key(|x| (x.label.starts_with("Untitled Proposal"), x.label.clone()));

        let your_edits = vec![
            Line("Your proposals").small_heading().into_widget(ctx),
            Menu::widget(ctx, your_proposals),
        ];
        // widgetry can't toggle keyboard focus between two menus, so just use buttons for the less
        // common use case.
        let mut proposals = vec![Line("Community proposals").small_heading().into_widget(ctx)];
        // Up-front filter out proposals that definitely don't fit the current map
        for name in abstio::list_all_objects(abstio::path("system/proposals")) {
            let path = abstio::path(format!("system/proposals/{}.json", name));
            if MapEdits::load_from_file(&app.primary.map, path.clone(), &mut Timer::throwaway())
                .is_ok()
            {
                proposals.push(ctx.style().btn_outline.text(name).build_widget(ctx, &path));
            }
        }

        Box::new(LoadEdits {
            mode,
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Load proposal").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                ctx.style()
                    .btn_outline
                    .text("Start over with blank proposal")
                    .build_def(ctx),
                Widget::row(vec![Widget::col(your_edits), Widget::col(proposals)]).evenly_spaced(),
            ]))
            .exact_size_percent(50, 50)
            .build(ctx),
        })
    }
}

impl State<App> for LoadEdits {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                match x.as_ref() {
                    "close" => Transition::Pop,
                    "Start over with blank proposal" => {
                        apply_map_edits(ctx, app, app.primary.map.new_edits());
                        Transition::Pop
                    }
                    path => {
                        // TODO Kind of a hack. If it ends with .json, it's already a path.
                        // Otherwise it's a result from the menu.
                        let path = if path.ends_with(".json") {
                            path.to_string()
                        } else {
                            abstio::path_edits(app.primary.map.get_name(), path)
                        };

                        match MapEdits::load_from_file(
                            &app.primary.map,
                            path.clone(),
                            &mut Timer::throwaway(),
                        )
                        .and_then(|edits| {
                            if self.mode.allows(&edits) {
                                Ok(edits)
                            } else {
                                Err(anyhow!(
                                    "The current gameplay mode restricts edits. This proposal has \
                                     a banned command."
                                ))
                            }
                        }) {
                            Ok(edits) => {
                                apply_map_edits(ctx, app, edits);
                                app.primary
                                    .sim
                                    .handle_live_edited_traffic_signals(&app.primary.map);
                                Transition::Pop
                            }
                            // TODO Hack. Have to replace ourselves, because the Menu might be
                            // invalidated now that something was chosen.
                            Err(err) => {
                                println!("Can't load {}: {}", path, err);
                                Transition::Multi(vec![
                                    Transition::Replace(LoadEdits::new_state(
                                        ctx,
                                        app,
                                        self.mode.clone(),
                                    )),
                                    // TODO Menu draws at a weird Z-order to deal with tooltips, so
                                    // now the menu underneath
                                    // bleeds through
                                    Transition::Push(PopupMsg::new_state(
                                        ctx,
                                        "Error",
                                        vec![format!("Can't load {}", path), err.to_string()],
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
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

fn make_topcenter(ctx: &mut EventCtx, app: &App) -> Panel {
    Panel::new_builder(Widget::col(vec![
        Line("Editing map")
            .small_heading()
            .into_widget(ctx)
            .centered_horiz(),
        ctx.style()
            .btn_solid_primary
            .text(format!(
                "Finish & resume from {}",
                app.primary
                    .suspended_sim
                    .as_ref()
                    .unwrap()
                    .time()
                    .ampm_tostring()
            ))
            .hotkey(Key::Escape)
            .build_widget(ctx, "finish editing"),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

pub fn apply_map_edits(ctx: &mut EventCtx, app: &mut App, edits: MapEdits) {
    let mut timer = Timer::new("apply map edits");

    if app.primary.unedited_map.is_none() {
        timer.start("save unedited map");
        assert!(app.primary.map.get_edits().commands.is_empty());
        app.primary.unedited_map = Some(app.primary.map.clone());
        timer.stop("save unedited map");
    }

    timer.start("edit map");
    let effects = app.primary.map.must_apply_edits(edits);
    timer.stop("edit map");

    if !effects.changed_roads.is_empty() || !effects.changed_intersections.is_empty() {
        app.primary
            .draw_map
            .draw_all_unzoomed_roads_and_intersections =
            DrawMap::regenerate_unzoomed_layer(&app.primary.map, &app.cs, ctx, &mut timer);
    }

    for l in effects.deleted_lanes {
        app.primary.draw_map.delete_lane(l);
    }

    for r in effects.changed_roads {
        let road = app.primary.map.get_r(r);
        app.primary.draw_map.recreate_road(road, &app.primary.map);

        // An edit to one lane potentially affects markings in all lanes in the same road, because
        // of one-way markings, driving lines, etc.
        for l in &road.lanes {
            app.primary.draw_map.create_lane(l.id, &app.primary.map);
        }
    }

    for i in effects.changed_intersections {
        app.primary
            .draw_map
            .recreate_intersection(i, &app.primary.map);
    }

    for pl in effects.changed_parking_lots {
        app.primary.draw_map.get_pl(pl).clear_rendering();
    }

    if app.primary.layer.as_ref().and_then(|l| l.name()) == Some("map edits") {
        app.primary.layer = Some(Box::new(crate::layer::map::Static::edits(ctx, app)));
    }

    // Autosave
    app.primary.map.save_edits();
}

pub fn can_edit_lane(app: &App, l: LaneID) -> bool {
    let map = &app.primary.map;
    !map.get_l(l).is_light_rail() && !map.get_parent(l).is_service()
}

pub fn speed_limit_choices(app: &App, preset: Option<Speed>) -> Vec<Choice<Speed>> {
    // Don't need anything higher than 70mph. Though now I kind of miss 3am drives on TX-71...
    let mut speeds = (10..=70)
        .step_by(5)
        .map(|mph| Speed::miles_per_hour(mph as f64))
        .collect::<Vec<_>>();
    if let Some(preset) = preset {
        if !speeds.contains(&preset) {
            speeds.push(preset);
            speeds.sort();
        }
    }
    speeds
        .into_iter()
        .map(|x| Choice::new(x.to_string(&app.opts.units), x))
        .collect()
}

pub fn maybe_edit_intersection(
    ctx: &mut EventCtx,
    app: &mut App,
    id: IntersectionID,
    mode: &GameplayMode,
) -> Option<Box<dyn State<App>>> {
    if app.primary.map.maybe_get_stop_sign(id).is_some()
        && mode.can_edit_stop_signs()
        && app.per_obj.left_click(ctx, "edit stop signs")
    {
        return Some(StopSignEditor::new_state(ctx, app, id, mode.clone()));
    }

    if app.primary.map.maybe_get_traffic_signal(id).is_some()
        && app.per_obj.left_click(ctx, "edit traffic signal")
    {
        return Some(TrafficSignalEditor::new_state(
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
            ctx.style()
                .btn_outline
                .popup(&edits.edits_name)
                .hotkey(lctrl(Key::P))
                .build_widget(ctx, "manage proposals"),
            "autosaved"
                .text_widget(ctx)
                .container()
                .padding(10)
                .bg(Color::hex("#5D9630")),
        ]),
        ColorLegend::row(
            ctx,
            app.cs.edits_layer,
            format!(
                "{} roads, {} intersections changed",
                edits.changed_roads.len(),
                edits.original_intersections.len()
            ),
        ),
    ];

    if edits.commands.len() > 5 {
        col.push(format!("{} more...", edits.commands.len() - 5).text_widget(ctx));
    }
    for idx in edits.commands.len().max(5) - 5..edits.commands.len() {
        let (summary, details) = edits.commands[idx].describe(&app.primary.map);
        let mut txt = Text::from(format!("{}) {}", idx + 1, summary));
        for line in details {
            txt.add_line(Line(line).secondary());
        }
        let btn = ctx
            .style()
            .btn_plain
            .btn()
            .label_styled_text(txt, ControlState::Default)
            .build_widget(ctx, format!("change #{}", idx + 1));
        if idx == edits.commands.len() - 1 {
            col.push(
                Widget::row(vec![
                    btn,
                    ctx.style()
                        .btn_close()
                        .hotkey(lctrl(Key::Z))
                        .build_widget(ctx, "undo"),
                ])
                .padding(16)
                .outline(ctx.style().btn_outline.outline),
            );
        } else {
            col.push(btn);
        }
    }

    Panel::new_builder(Widget::col(col))
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

pub struct ConfirmDiscard {
    panel: Panel,
    discard: Box<dyn Fn(&mut App)>,
}

impl ConfirmDiscard {
    pub fn new_state(ctx: &mut EventCtx, discard: Box<dyn Fn(&mut App)>) -> Box<dyn State<App>> {
        Box::new(ConfirmDiscard {
            discard,
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Image::from_path("system/assets/tools/alert.svg")
                        .untinted()
                        .into_widget(ctx)
                        .container()
                        .padding_top(6),
                    Line("Alert").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                "Are you sure you want to discard changes you made?".text_widget(ctx),
                Widget::row(vec![
                    ctx.style()
                        .btn_outline
                        .text("Cancel")
                        .hotkey(Key::Escape)
                        .build_def(ctx),
                    ctx.style()
                        .btn_solid_destructive
                        .text("Yes, discard")
                        .build_def(ctx),
                ])
                .align_right(),
            ]))
            .build(ctx),
        })
    }
}

impl State<App> for ConfirmDiscard {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" | "Cancel" => Transition::Pop,
                "Yes, discard" => {
                    (self.discard)(app);
                    Transition::Multi(vec![Transition::Pop, Transition::Pop])
                }
                _ => unreachable!(),
            },
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}

use geom::{Distance, Line, Polygon, Pt2D};
use raw_map::osm;
use widgetry::mapspace::WorldOutcome;
use widgetry::tools::{open_browser, PopupMsg, URLManager};
use widgetry::{
    lctrl, Canvas, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    SharedAppState, State, Text, Toggle, Transition, VerticalAlignment, Widget,
};

use crate::camera::CameraState;
use crate::model::{Model, ID};

pub struct App {
    pub model: Model,
}

impl SharedAppState for App {
    fn draw_default(&self, g: &mut GfxCtx) {
        g.clear(Color::BLACK);
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        if !self.model.map.name.map.is_empty() {
            CameraState::save(canvas, &self.model.map.name);
        }
    }

    fn before_quit(&self, canvas: &Canvas) {
        if !self.model.map.name.map.is_empty() {
            CameraState::save(canvas, &self.model.map.name);
        }
    }
}

pub struct MainState {
    mode: Mode,
    panel: Panel,
}

enum Mode {
    Neutral,
    CreatingRoad(osm::NodeID),
    SetBoundaryPt1,
    SetBoundaryPt2(Pt2D),
}

impl MainState {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        if !app.model.map.name.map.is_empty() {
            URLManager::update_url_free_param(
                abstio::path_raw_map(&app.model.map.name)
                    .strip_prefix(&abstio::path(""))
                    .unwrap()
                    .to_string(),
            );
        }
        let bounds = app.model.map.gps_bounds.to_bounds();
        ctx.canvas.map_dims = (bounds.width(), bounds.height());

        let mut state = MainState {
            mode: Mode::Neutral,
            panel: Panel::new_builder(Widget::col(vec![
                Line("RawMap Editor").small_heading().into_widget(ctx),
                Widget::col(vec![
                    Widget::col(vec![
                        Widget::row(vec![
                            ctx.style()
                                .btn_popup_icon_text(
                                    "system/assets/tools/map.svg",
                                    &app.model.map.name.as_filename(),
                                )
                                .hotkey(lctrl(Key::L))
                                .build_widget(ctx, "open another RawMap"),
                            ctx.style()
                                .btn_solid_destructive
                                .text("reload")
                                .build_def(ctx),
                        ]),
                        if cfg!(target_arch = "wasm32") {
                            Widget::nothing()
                        } else {
                            Widget::row(vec![
                                ctx.style()
                                    .btn_solid_primary
                                    .text("export to OSM")
                                    .build_def(ctx),
                                ctx.style()
                                    .btn_solid_destructive
                                    .text("overwrite RawMap")
                                    .build_def(ctx),
                            ])
                        },
                    ])
                    .section(ctx),
                    Widget::col(vec![
                        Toggle::choice(ctx, "create", "intersection", "building", None, true),
                        Toggle::switch(ctx, "show intersection geometry", Key::G, false),
                        ctx.style()
                            .btn_outline
                            .text("adjust boundary")
                            .build_def(ctx),
                        ctx.style()
                            .btn_outline
                            .text("detect short roads")
                            .build_def(ctx),
                        ctx.style()
                            .btn_outline
                            .text("simplify RawMap")
                            .build_def(ctx),
                        ctx.style()
                            .btn_outline
                            .text("save osm2polygons input")
                            .build_def(ctx),
                    ])
                    .section(ctx),
                ]),
                Text::new().into_widget(ctx).named("instructions"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        };
        state.update_instructions(ctx, app);
        Box::new(state)
    }

    fn update_instructions(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut txt = Text::new();
        if let Some(keybindings) = app.model.world.get_hovered_keybindings() {
            // TODO Should we also say click and drag to move it? Or for clickable roads, click to
            // edit?
            for (key, action) in keybindings {
                txt.add_appended(vec![
                    Line("- Press "),
                    key.txt(ctx),
                    Line(format!(" to {}", action)),
                ]);
            }
        } else {
            txt.add_appended(vec![
                Line("Click").fg(ctx.style().text_hotkey_color),
                Line(" to create a new intersection or building"),
            ]);
        }
        let instructions = txt.into_widget(ctx);
        self.panel.replace(ctx, "instructions", instructions);
    }
}

impl State<App> for MainState {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        match self.mode {
            Mode::Neutral => {
                // TODO Update URL when canvas moves
                match app.model.world.event(ctx) {
                    WorldOutcome::ClickedFreeSpace(pt) => {
                        if self.panel.is_checked("create") {
                            app.model.create_i(ctx, pt);
                        } else {
                            app.model.create_b(ctx, pt);
                        }
                        app.model.world.initialize_hover(ctx);
                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Dragging {
                        obj: ID::Intersection(i),
                        cursor,
                        ..
                    } => {
                        app.model.move_i(ctx, i, cursor);
                    }
                    WorldOutcome::Dragging {
                        obj: ID::Building(b),
                        dx,
                        dy,
                        ..
                    } => {
                        app.model.move_b(ctx, b, dx, dy);
                    }
                    WorldOutcome::Dragging {
                        obj: ID::RoadPoint(r, idx),
                        cursor,
                        ..
                    } => {
                        app.model.move_r_pt(ctx, r, idx, cursor);
                    }
                    WorldOutcome::HoverChanged(before, after) => {
                        if let Some(ID::Road(r)) | Some(ID::RoadPoint(r, _)) = before {
                            app.model.stop_showing_pts(r);
                        }
                        if let Some(ID::Road(r)) | Some(ID::RoadPoint(r, _)) = after {
                            app.model.show_r_points(ctx, r);
                            // Shouldn't need to call initialize_hover, unless the user somehow
                            // warped their cursor to precisely the location of a point, in the
                            // middle of the road!
                        }

                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Keypress("start a road here", ID::Intersection(i)) => {
                        self.mode = Mode::CreatingRoad(i);
                    }
                    WorldOutcome::Keypress("delete", ID::Intersection(i)) => {
                        app.model.delete_i(i);
                        app.model.world.initialize_hover(ctx);
                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Keypress(
                        "toggle stop sign / traffic signal",
                        ID::Intersection(i),
                    ) => {
                        app.model.toggle_i(ctx, i);
                    }
                    WorldOutcome::Keypress("debug intersection geometry", ID::Intersection(i)) => {
                        app.model.debug_intersection_geometry(ctx, i);
                    }
                    WorldOutcome::Keypress("export to osm2polygon", ID::Intersection(i)) => {
                        let input = format!("{}_input.json", i.0);
                        let output = format!("{}_output.json", i.0);

                        return Transition::Push(
                            match app
                                .model
                                .map
                                .save_osm2polygon_input(input.clone(), i)
                                .and_then(|_| {
                                    raw_map::geometry::osm2polygon(input.clone(), output.clone())
                                }) {
                                Ok(()) => PopupMsg::new_state(
                                    ctx,
                                    "Exported",
                                    vec![format!("{input} and {output} written")],
                                ),
                                Err(err) => {
                                    PopupMsg::new_state(ctx, "Error", vec![err.to_string()])
                                }
                            },
                        );
                    }
                    WorldOutcome::Keypress("debug in OSM", ID::Intersection(i)) => {
                        open_browser(i.to_string());
                    }
                    WorldOutcome::Keypress("delete", ID::Building(b)) => {
                        app.model.delete_b(b);
                        app.model.world.initialize_hover(ctx);
                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Keypress("delete", ID::Road(r)) => {
                        app.model.delete_r(ctx, r);
                        // There may be something underneath the road, so recalculate immediately
                        app.model.world.initialize_hover(ctx);
                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Keypress("insert a new point here", ID::Road(r)) => {
                        if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                            app.model.insert_r_pt(ctx, r, pt);
                            app.model.world.initialize_hover(ctx);
                            self.update_instructions(ctx, app);
                        }
                    }
                    WorldOutcome::Keypress("remove interior points", ID::Road(r)) => {
                        app.model.clear_r_pts(ctx, r);
                        app.model.world.initialize_hover(ctx);
                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Keypress("delete", ID::RoadPoint(r, idx)) => {
                        app.model.delete_r_pt(ctx, r, idx);
                        app.model.world.initialize_hover(ctx);
                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Keypress("merge", ID::Road(r)) => {
                        app.model.merge_r(ctx, r);
                        app.model.world.initialize_hover(ctx);
                        self.update_instructions(ctx, app);
                    }
                    WorldOutcome::Keypress("mark/unmark as a junction", ID::Road(r)) => {
                        app.model.toggle_junction(ctx, r);
                    }
                    WorldOutcome::Keypress("debug in OSM", ID::Road(r)) => {
                        open_browser(r.osm_way_id.to_string());
                    }
                    WorldOutcome::ClickedObject(ID::Road(r)) => {
                        return Transition::Push(crate::edit::EditRoad::new_state(ctx, app, r));
                    }
                    _ => {}
                }

                match self.panel.event(ctx) {
                    Outcome::Clicked(x) => match x.as_ref() {
                        "adjust boundary" => {
                            self.mode = Mode::SetBoundaryPt1;
                        }
                        "detect short roads" => {
                            for r in app.model.map.find_dog_legs() {
                                app.model.road_deleted(r);
                                app.model.road_added(ctx, r);
                            }
                        }
                        "simplify RawMap" => {
                            ctx.loading_screen("simplify", |ctx, timer| {
                                app.model.map.run_all_simplifications(false, timer);
                                app.model.recreate_world(ctx, timer);
                            });
                        }
                        "export to OSM" => {
                            app.model.export_to_osm();
                        }
                        "overwrite RawMap" => {
                            app.model.map.save();
                        }
                        "reload" => {
                            CameraState::save(ctx.canvas, &app.model.map.name);
                            return Transition::Push(crate::load::load_map(
                                ctx,
                                abstio::path_raw_map(&app.model.map.name),
                                app.model.include_bldgs,
                                None,
                            ));
                        }
                        "open another RawMap" => {
                            CameraState::save(ctx.canvas, &app.model.map.name);
                            return Transition::Push(crate::load::PickMap::new_state(ctx));
                        }
                        _ => unreachable!(),
                    },
                    Outcome::Changed(_) => {
                        app.model.show_intersection_geometry(
                            ctx,
                            self.panel.is_checked("show intersection geometry"),
                        );
                    }
                    _ => {}
                }
            }
            Mode::CreatingRoad(i1) => {
                if ctx.canvas_movement() {
                    URLManager::update_url_cam(ctx, &app.model.map.gps_bounds);
                }

                if ctx.input.pressed(Key::Escape) {
                    self.mode = Mode::Neutral;
                    // TODO redo mouseover?
                } else if let Some(ID::Intersection(i2)) = app.model.world.calculate_hovering(ctx) {
                    if i1 != i2 && ctx.input.pressed(Key::R) {
                        app.model.create_r(ctx, i1, i2);
                        self.mode = Mode::Neutral;
                        // TODO redo mouseover?
                    }
                }
            }
            Mode::SetBoundaryPt1 => {
                if ctx.canvas_movement() {
                    URLManager::update_url_cam(ctx, &app.model.map.gps_bounds);
                }

                let mut txt = Text::new();
                txt.add_appended(vec![
                    Line("Click").fg(ctx.style().text_hotkey_color),
                    Line(" the top-left corner of this map"),
                ]);
                let instructions = txt.into_widget(ctx);
                self.panel.replace(ctx, "instructions", instructions);

                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if ctx.normal_left_click() {
                        self.mode = Mode::SetBoundaryPt2(pt);
                    }
                }
            }
            Mode::SetBoundaryPt2(pt1) => {
                if ctx.canvas_movement() {
                    URLManager::update_url_cam(ctx, &app.model.map.gps_bounds);
                }

                let mut txt = Text::new();
                txt.add_appended(vec![
                    Line("Click").fg(ctx.style().text_hotkey_color),
                    Line(" the bottom-right corner of this map"),
                ]);
                let instructions = txt.into_widget(ctx);
                self.panel.replace(ctx, "instructions", instructions);

                if let Some(pt2) = ctx.canvas.get_cursor_in_map_space() {
                    if ctx.normal_left_click() {
                        app.model.set_boundary(ctx, pt1, pt2);
                        self.mode = Mode::Neutral;
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // It's useful to see the origin.
        g.draw_polygon(Color::WHITE, Polygon::rectangle(100.0, 10.0));
        g.draw_polygon(Color::WHITE, Polygon::rectangle(10.0, 100.0));

        g.draw_polygon(
            Color::rgb(242, 239, 233),
            app.model.map.boundary_polygon.clone(),
        );
        app.model.world.draw(g);
        g.redraw(&app.model.draw_extra);

        match self.mode {
            Mode::Neutral | Mode::SetBoundaryPt1 => {}
            Mode::CreatingRoad(i1) => {
                if let Some(cursor) = g.get_cursor_in_map_space() {
                    if let Ok(l) = Line::new(app.model.map.intersections[&i1].point, cursor) {
                        g.draw_polygon(Color::GREEN, l.make_polygons(Distance::meters(5.0)));
                    }
                }
            }
            Mode::SetBoundaryPt2(pt1) => {
                if let Some(pt2) = g.canvas.get_cursor_in_map_space() {
                    if let Some(rect) = Polygon::rectangle_two_corners(pt1, pt2) {
                        g.draw_polygon(Color::YELLOW.alpha(0.5), rect);
                    }
                }
            }
        };

        self.panel.draw(g);
    }
}

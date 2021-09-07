use geom::{Distance, Line, Polygon, Pt2D};
use map_gui::tools::{CameraState, URLManager};
use map_gui::AppLike;
use map_model::osm;
use map_model::raw::OriginalRoad;
use widgetry::{
    lctrl, Canvas, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    SharedAppState, State, Text, Toggle, Transition, VerticalAlignment, Widget,
};

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

// We use a few things from map_gui that don't actually require these... maybe meaning they should
// be lifted even further to widgetry.

impl AppLike for App {
    fn map(&self) -> &map_model::Map {
        unreachable!()
    }
    fn sim(&self) -> &sim::Sim {
        unreachable!()
    }
    fn cs(&self) -> &map_gui::colors::ColorScheme {
        unreachable!()
    }
    fn mut_cs(&mut self) -> &mut map_gui::colors::ColorScheme {
        unreachable!()
    }
    fn draw_map(&self) -> &map_gui::render::DrawMap {
        unreachable!()
    }
    fn mut_draw_map(&mut self) -> &mut map_gui::render::DrawMap {
        unreachable!()
    }
    fn opts(&self) -> &map_gui::options::Options {
        unreachable!()
    }
    fn mut_opts(&mut self) -> &mut map_gui::options::Options {
        unreachable!()
    }
    fn map_switched(&mut self, _: &mut EventCtx, _: map_model::Map, _: &mut abstutil::Timer) {
        unreachable!()
    }
    fn draw_with_opts(&self, _: &mut GfxCtx, _: map_gui::render::DrawOptions) {
        unreachable!()
    }
    fn make_warper(
        &mut self,
        _: &EventCtx,
        _: Pt2D,
        _: Option<f64>,
        _: Option<map_gui::ID>,
    ) -> Box<dyn State<App>> {
        unreachable!()
    }
    fn sim_time(&self) -> geom::Time {
        unreachable!()
    }
    fn current_stage_and_remaining_time(
        &self,
        _: map_model::IntersectionID,
    ) -> (usize, geom::Duration) {
        unreachable!()
    }
}

pub struct MainState {
    mode: Mode,
    panel: Panel,

    last_id: Option<ID>,
}

enum Mode {
    Viewing,
    MovingIntersection(osm::NodeID),
    MovingBuilding(osm::OsmID),
    MovingRoadPoint(OriginalRoad, usize),
    CreatingRoad(osm::NodeID),
    SetBoundaryPt1,
    SetBoundaryPt2(Pt2D),
}

impl MainState {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        if !app.model.map.name.map.is_empty() {
            if let Err(err) = URLManager::update_url_free_param(
                abstio::path_raw_map(&app.model.map.name)
                    .strip_prefix(&abstio::path(""))
                    .unwrap()
                    .to_string(),
            ) {
                warn!("Couldn't update URL: {}", err);
            }
        }
        let bounds = app.model.map.gps_bounds.to_bounds();
        ctx.canvas.map_dims = (bounds.width(), bounds.height());

        // TODO Make these dynamic!
        let mut instructions = Text::new();
        instructions.add_appended(vec![
            Line("Press "),
            Key::I.txt(ctx),
            Line(" to create a new intersection"),
        ]);
        instructions.add_line("Hover on an intersection, then...");
        instructions.add_appended(vec![
            Line("- Press "),
            Key::R.txt(ctx),
            Line(" to start/end a new road"),
        ]);
        instructions.add_appended(vec![
            Line("- Hold "),
            Key::LeftControl.txt(ctx),
            Line(" to move it"),
        ]);
        instructions.add_appended(vec![
            Line("Press "),
            Key::Backspace.txt(ctx),
            Line(" to delete something"),
        ]);

        Box::new(MainState {
            mode: Mode::Viewing,
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
                        Toggle::switch(ctx, "intersection geometry", Key::G, false),
                        ctx.style()
                            .btn_outline
                            .text("adjust boundary")
                            .build_def(ctx),
                        ctx.style()
                            .btn_outline
                            .text("auto mark junctions")
                            .build_def(ctx),
                        ctx.style()
                            .btn_outline
                            .text("simplify RawMap")
                            .build_def(ctx),
                    ])
                    .section(ctx),
                ]),
                Text::new().into_widget(ctx).named("instructions"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),

            last_id: None,
        })
    }
}

impl State<App> for MainState {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        if ctx.canvas_movement() {
            if let Err(err) = URLManager::update_url_cam(ctx, &app.model.map.gps_bounds) {
                warn!("Couldn't update URL: {}", err);
            }
        }
        if ctx.redo_mouseover() {
            app.model.world.handle_mouseover(ctx);
        }

        let mut cursor = ctx.canvas.get_cursor_in_map_space();
        // Negative coordinates break the quadtree in World, so try to prevent anything involving
        // them. Creating stuff near the boundary or moving things past it still crash, but this
        // and drawing the boundary kind of help.
        if let Some(pt) = cursor {
            if pt.x() < 0.0 || pt.y() < 0.0 {
                cursor = None;
            }
        }

        match self.mode {
            Mode::Viewing => {
                {
                    let before = match self.last_id {
                        Some(ID::Road(r)) | Some(ID::RoadPoint(r, _)) => Some(r),
                        _ => None,
                    };
                    let after = match app.model.world.get_selection() {
                        Some(ID::Road(r)) | Some(ID::RoadPoint(r, _)) => Some(r),
                        _ => None,
                    };
                    if before != after {
                        if let Some(id) = before {
                            app.model.stop_showing_pts(id);
                        }
                        if let Some(r) = after {
                            app.model.show_r_points(ctx, r);
                            app.model.world.handle_mouseover(ctx);
                        }
                    }
                }

                match app.model.world.get_selection() {
                    Some(ID::Intersection(i)) => {
                        if ctx.input.pressed(Key::LeftControl) {
                            self.mode = Mode::MovingIntersection(i);
                        } else if ctx.input.pressed(Key::R) {
                            self.mode = Mode::CreatingRoad(i);
                        } else if ctx.input.pressed(Key::Backspace) {
                            app.model.delete_i(i);
                            app.model.world.handle_mouseover(ctx);
                        } else if ctx.input.pressed(Key::T) {
                            app.model.toggle_i(ctx, i);
                        } else if ctx.input.pressed(Key::P) {
                            app.model.debug_intersection_geometry(ctx, i);
                        }

                        let mut txt = Text::new();
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::R.txt(ctx),
                            Line(" to start a road here"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::Backspace.txt(ctx),
                            Line(" to delete"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Hold "),
                            Key::LeftControl.txt(ctx),
                            Line(" to move"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::T.txt(ctx),
                            Line(" to toggle stop sign / traffic signal"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::P.txt(ctx),
                            Line(" to debug intersection geometry"),
                        ]);
                        let instructions = txt.into_widget(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    Some(ID::Building(b)) => {
                        if ctx.input.pressed(Key::LeftControl) {
                            self.mode = Mode::MovingBuilding(b);
                        } else if ctx.input.pressed(Key::Backspace) {
                            app.model.delete_b(b);
                            app.model.world.handle_mouseover(ctx);
                        }

                        let mut txt = Text::new();
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::Backspace.txt(ctx),
                            Line(" to delete"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Hold "),
                            Key::LeftControl.txt(ctx),
                            Line(" to move"),
                        ]);
                        let instructions = txt.into_widget(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    Some(ID::Road(r)) => {
                        if ctx.input.pressed(Key::Backspace) {
                            app.model.delete_r(ctx, r);
                            app.model.world.handle_mouseover(ctx);
                        } else if cursor.is_some() && ctx.input.pressed(Key::P) {
                            if let Some(id) = app.model.insert_r_pt(ctx, r, cursor.unwrap()) {
                                app.model.world.force_set_selection(id);
                            }
                        } else if ctx.input.pressed(Key::X) {
                            app.model.clear_r_pts(ctx, r);
                        } else if ctx.input.pressed(Key::M) {
                            app.model.merge_r(ctx, r);
                            app.model.world.handle_mouseover(ctx);
                        } else if ctx.input.pressed(Key::J) {
                            app.model.toggle_junction(ctx, r);
                        } else if ctx.normal_left_click() {
                            return Transition::Push(crate::edit::EditRoad::new_state(ctx, app, r));
                        }

                        let mut txt = Text::new();
                        txt.add_appended(vec![
                            Line("Click").fg(ctx.style().text_hotkey_color),
                            Line(" to edit lanes"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::Backspace.txt(ctx),
                            Line(" to delete"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::P.txt(ctx),
                            Line(" to insert a new point here"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::X.txt(ctx),
                            Line(" to remove interior points"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::M.txt(ctx),
                            Line(" to merge"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::J.txt(ctx),
                            Line(" to mark/unmark as a junction"),
                        ]);
                        let instructions = txt.into_widget(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    Some(ID::RoadPoint(r, idx)) => {
                        if ctx.input.pressed(Key::LeftControl) {
                            self.mode = Mode::MovingRoadPoint(r, idx);
                        } else if ctx.input.pressed(Key::Backspace) {
                            app.model.delete_r_pt(ctx, r, idx);
                            app.model.world.handle_mouseover(ctx);
                        }

                        let mut txt = Text::new();
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::Backspace.txt(ctx),
                            Line(" to delete"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Hold "),
                            Key::LeftControl.txt(ctx),
                            Line(" to move"),
                        ]);
                        let instructions = txt.into_widget(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    None => {
                        match self.panel.event(ctx) {
                            Outcome::Clicked(x) => match x.as_ref() {
                                "adjust boundary" => {
                                    self.mode = Mode::SetBoundaryPt1;
                                }
                                "auto mark junctions" => {
                                    for r in app.model.map.auto_mark_junctions() {
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
                                    CameraState::save(&ctx.canvas, &app.model.map.name);
                                    return Transition::Push(crate::load::load_map(
                                        ctx,
                                        abstio::path_raw_map(&app.model.map.name),
                                        app.model.include_bldgs,
                                        None,
                                    ));
                                }
                                "open another RawMap" => {
                                    CameraState::save(&ctx.canvas, &app.model.map.name);
                                    return Transition::Push(crate::load::PickMap::new_state(ctx));
                                }
                                _ => unreachable!(),
                            },
                            Outcome::Changed(_) => {
                                app.model.show_intersection_geometry(
                                    ctx,
                                    self.panel.is_checked("intersection geometry"),
                                );
                            }
                            _ => {
                                if ctx.input.pressed(Key::I) {
                                    if let Some(pt) = cursor {
                                        app.model.create_i(ctx, pt);
                                        app.model.world.handle_mouseover(ctx);
                                    }
                                // TODO Silly bug: Mouseover doesn't actually work! I think the
                                // cursor being dead-center messes
                                // up the precomputed triangles.
                                } else if ctx.input.pressed(Key::B) {
                                    if let Some(pt) = cursor {
                                        let id = app.model.create_b(ctx, pt);
                                        app.model.world.force_set_selection(id);
                                    }
                                }

                                let mut txt = Text::new();
                                txt.add_appended(vec![
                                    Line("- Press "),
                                    Key::I.txt(ctx),
                                    Line(" to create an intersection"),
                                ]);
                                txt.add_appended(vec![
                                    Line("- Press "),
                                    Key::B.txt(ctx),
                                    Line(" to create a building"),
                                ]);
                                let instructions = txt.into_widget(ctx);
                                self.panel.replace(ctx, "instructions", instructions);
                            }
                        }
                    }
                }
            }
            Mode::MovingIntersection(id) => {
                if let Some(pt) = cursor {
                    app.model.move_i(ctx, id, pt);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.mode = Mode::Viewing;
                    }
                }
            }
            Mode::MovingBuilding(id) => {
                if let Some(pt) = cursor {
                    app.model.move_b(ctx, id, pt);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.mode = Mode::Viewing;
                    }
                }
            }
            Mode::MovingRoadPoint(r, idx) => {
                if let Some(pt) = cursor {
                    app.model.move_r_pt(ctx, r, idx, pt);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.mode = Mode::Viewing;
                    }
                }
            }
            Mode::CreatingRoad(i1) => {
                if ctx.input.pressed(Key::Escape) {
                    self.mode = Mode::Viewing;
                    app.model.world.handle_mouseover(ctx);
                } else if let Some(ID::Intersection(i2)) = app.model.world.get_selection() {
                    if i1 != i2 && ctx.input.pressed(Key::R) {
                        app.model.create_r(ctx, i1, i2);
                        self.mode = Mode::Viewing;
                        app.model.world.handle_mouseover(ctx);
                    }
                }
            }
            Mode::SetBoundaryPt1 => {
                let mut txt = Text::new();
                txt.add_appended(vec![
                    Line("Click").fg(ctx.style().text_hotkey_color),
                    Line(" the top-left corner of this map"),
                ]);
                let instructions = txt.into_widget(ctx);
                self.panel.replace(ctx, "instructions", instructions);

                if let Some(pt) = cursor {
                    if ctx.normal_left_click() {
                        self.mode = Mode::SetBoundaryPt2(pt);
                    }
                }
            }
            Mode::SetBoundaryPt2(pt1) => {
                let mut txt = Text::new();
                txt.add_appended(vec![
                    Line("Click").fg(ctx.style().text_hotkey_color),
                    Line(" the bottom-right corner of this map"),
                ]);
                let instructions = txt.into_widget(ctx);
                self.panel.replace(ctx, "instructions", instructions);

                if let Some(pt2) = cursor {
                    if ctx.normal_left_click() {
                        app.model.set_boundary(ctx, pt1, pt2);
                        self.mode = Mode::Viewing;
                    }
                }
            }
        }

        self.last_id = app.model.world.get_selection();

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
        app.model.world.draw(g, |_| true);
        g.redraw(&app.model.draw_extra);

        match self.mode {
            Mode::CreatingRoad(i1) => {
                if let Some(cursor) = g.get_cursor_in_map_space() {
                    if let Some(l) = Line::new(app.model.map.intersections[&i1].point, cursor) {
                        g.draw_polygon(Color::GREEN, l.make_polygons(Distance::meters(5.0)));
                    }
                }
            }
            Mode::Viewing
            | Mode::MovingIntersection(_)
            | Mode::MovingBuilding(_)
            | Mode::MovingRoadPoint(_, _) => {}
            Mode::SetBoundaryPt1 => {}
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

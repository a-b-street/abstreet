//! The map_editor renders and lets you edit RawMaps, which are a format in between OSM and the
//! full Map. It's useful for debugging maps imported from OSM, and for drawing synthetic maps for
//! testing.

#[macro_use]
extern crate log;

use model::{Model, ID};

use abstutil::{CmdArgs, Timer};
use geom::{Distance, Line, Polygon};
use map_gui::tools::CameraState;
use map_model::osm;
use map_model::raw::OriginalRoad;
use widgetry::{
    Canvas, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, SharedAppState, State, StyledButtons, Text, Transition, VerticalAlignment, Widget,
};

mod edit;
mod model;
mod world;

struct App {
    model: Model,
}

impl SharedAppState for App {
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

struct MainState {
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
    PreviewIntersection(Drawable),
}

impl MainState {
    fn new(ctx: &mut EventCtx) -> (App, MainState) {
        let mut args = CmdArgs::new();
        let load = args.optional_free();
        let include_bldgs = args.enabled("--bldgs");
        let intersection_geom = args.enabled("--geom");
        args.done();

        let model = if let Some(path) = load {
            Model::import(path, include_bldgs, intersection_geom, ctx)
        } else {
            Model::blank()
        };
        if !model.map.name.map.is_empty() {
            CameraState::load(ctx, &model.map.name);
        }
        let bounds = model.map.gps_bounds.to_bounds();
        ctx.canvas.map_dims = (bounds.width(), bounds.height());

        // TODO Make these dynamic!
        let mut instructions = Text::new();
        instructions.add_appended(vec![
            Line("Press "),
            Key::I.txt(ctx),
            Line(" to create a new intersection"),
        ]);
        instructions.add(Line("Hover on an intersection, then..."));
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

        (
            App { model },
            MainState {
                mode: Mode::Viewing,
                panel: Panel::new(Widget::col(vec![
                    Widget::row(vec![
                        Line("Map Editor").small_heading().draw(ctx),
                        ctx.style().btn_close_widget(ctx),
                    ]),
                    Text::new().draw(ctx).named("instructions"),
                    Widget::col(vec![
                        ctx.style()
                            .btn_solid_dark_text("export to OSM")
                            .build_def(ctx),
                        ctx.style()
                            .btn_outline_light_text("preview all intersections")
                            .build_def(ctx),
                    ]),
                ]))
                .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
                .build(ctx),

                last_id: None,
            },
        )
    }
}

impl State<App> for MainState {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();
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
                            app.model.show_r_points(r, ctx);
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
                        } else if !app.model.intersection_geom && ctx.input.pressed(Key::P) {
                            let draw = preview_intersection(i, &app.model, ctx);
                            self.mode = Mode::PreviewIntersection(draw);
                        } else if ctx.input.pressed(Key::T) {
                            app.model.toggle_i(ctx, i);
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
                            Key::P.txt(ctx),
                            Line(" to preview geometry"),
                        ]);
                        txt.add_appended(vec![
                            Line("- Press "),
                            Key::T.txt(ctx),
                            Line(" to toggle stop sign / traffic signal"),
                        ]);
                        let instructions = txt.draw(ctx);
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
                        let instructions = txt.draw(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    Some(ID::Road(r)) => {
                        if ctx.input.pressed(Key::Backspace) {
                            app.model.delete_r(r);
                            app.model.world.handle_mouseover(ctx);
                        } else if cursor.is_some() && ctx.input.pressed(Key::P) {
                            if let Some(id) = app.model.insert_r_pt(r, cursor.unwrap(), ctx) {
                                app.model.world.force_set_selection(id);
                            }
                        } else if ctx.input.pressed(Key::X) {
                            app.model.clear_r_pts(r, ctx);
                        } else if ctx.input.pressed(Key::M) {
                            app.model.merge_r(r, ctx);
                            app.model.world.handle_mouseover(ctx);
                        } else if ctx.normal_left_click() {
                            return Transition::Push(edit::EditRoad::new(ctx, app, r));
                        }

                        let mut txt = Text::new();
                        txt.add_appended(vec![
                            Line("Click").fg(ctx.style().hotkey_color),
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
                        let instructions = txt.draw(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    Some(ID::RoadPoint(r, idx)) => {
                        if ctx.input.pressed(Key::LeftControl) {
                            self.mode = Mode::MovingRoadPoint(r, idx);
                        } else if ctx.input.pressed(Key::Backspace) {
                            app.model.delete_r_pt(r, idx, ctx);
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
                        let instructions = txt.draw(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    None => {
                        match self.panel.event(ctx) {
                            Outcome::Clicked(x) => match x.as_ref() {
                                "close" => {
                                    return Transition::Pop;
                                }
                                "export to OSM" => {
                                    app.model.export_to_osm();
                                }
                                "preview all intersections" => {
                                    if !app.model.intersection_geom {
                                        let draw = preview_all_intersections(&app.model, ctx);
                                        self.mode = Mode::PreviewIntersection(draw);
                                    }
                                }
                                _ => unreachable!(),
                            },
                            _ => {
                                if ctx.input.pressed(Key::I) {
                                    if let Some(pt) = cursor {
                                        app.model.create_i(pt, ctx);
                                        app.model.world.handle_mouseover(ctx);
                                    }
                                // TODO Silly bug: Mouseover doesn't actually work! I think the
                                // cursor being dead-center messes
                                // up the precomputed triangles.
                                } else if ctx.input.pressed(Key::B) {
                                    if let Some(pt) = cursor {
                                        let id = app.model.create_b(pt, ctx);
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
                                let instructions = txt.draw(ctx);
                                self.panel.replace(ctx, "instructions", instructions);
                            }
                        }
                    }
                }
            }
            Mode::MovingIntersection(id) => {
                if let Some(pt) = cursor {
                    app.model.move_i(id, pt, ctx);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.mode = Mode::Viewing;
                    }
                }
            }
            Mode::MovingBuilding(id) => {
                if let Some(pt) = cursor {
                    app.model.move_b(id, pt, ctx);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.mode = Mode::Viewing;
                    }
                }
            }
            Mode::MovingRoadPoint(r, idx) => {
                if let Some(pt) = cursor {
                    app.model.move_r_pt(r, idx, pt, ctx);
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
                        app.model.create_r(i1, i2, ctx);
                        self.mode = Mode::Viewing;
                        app.model.world.handle_mouseover(ctx);
                    }
                }
            }
            Mode::PreviewIntersection(_) => {
                if ctx.input.pressed(Key::P) {
                    self.mode = Mode::Viewing;
                    app.model.world.handle_mouseover(ctx);
                }
            }
        }

        self.last_id = app.model.world.get_selection();

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(Color::BLACK);

        // It's useful to see the origin.
        g.draw_polygon(Color::WHITE, Polygon::rectangle(100.0, 10.0));
        g.draw_polygon(Color::WHITE, Polygon::rectangle(10.0, 100.0));

        g.draw_polygon(
            Color::rgb(242, 239, 233),
            app.model.map.boundary_polygon.clone(),
        );
        match self.mode {
            Mode::PreviewIntersection(_) => app.model.world.draw(g, |id| match id {
                ID::Intersection(_) => false,
                _ => true,
            }),
            _ => app.model.world.draw(g, |_| true),
        }

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
            Mode::PreviewIntersection(ref draw) => {
                g.redraw(draw);

                if g.is_key_down(Key::RightAlt) {
                    // TODO Argh, covers up mouseover tooltip.
                    if let Some(cursor) = g.canvas.get_cursor_in_map_space() {
                        g.draw_mouse_tooltip(Text::from(Line(cursor.to_string())));
                    }
                }
            }
        };

        self.panel.draw(g);
    }
}

fn preview_intersection(i: osm::NodeID, model: &Model, ctx: &EventCtx) -> Drawable {
    let (intersection, roads, debug) = model.map.preview_intersection(i);
    let mut batch = GeomBatch::new();
    batch.push(Color::ORANGE.alpha(0.5), intersection);
    for r in roads {
        batch.push(Color::GREEN.alpha(0.5), r);
    }
    for (label, poly) in debug {
        let center = poly.center();
        batch.push(Color::RED.alpha(0.5), poly);
        batch.append(
            Text::from(Line(label))
                .with_bg()
                .render_autocropped(ctx)
                .scale(0.1)
                .centered_on(center),
        );
    }
    batch.upload(ctx)
}

fn preview_all_intersections(model: &Model, ctx: &EventCtx) -> Drawable {
    let mut batch = GeomBatch::new();
    let mut timer = Timer::new("preview all intersections");
    timer.start_iter("preview", model.map.intersections.len());
    for i in model.map.intersections.keys() {
        timer.next();
        if model.map.roads_per_intersection(*i).is_empty() {
            continue;
        }
        let (intersection, _, _) = model.map.preview_intersection(*i);
        batch.push(Color::ORANGE.alpha(0.5), intersection);
    }
    batch.upload(ctx)
}

fn main() {
    widgetry::run(
        widgetry::Settings::new("RawMap editor").read_svg(Box::new(abstio::slurp_bytes)),
        |ctx| {
            let (app, state) = MainState::new(ctx);
            (app, vec![Box::new(state)])
        },
    );
}

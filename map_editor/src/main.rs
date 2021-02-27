//! The map_editor renders and lets you edit RawMaps, which are a format in between OSM and the
//! full Map. It's useful for debugging maps imported from OSM, and for drawing synthetic maps for
//! testing.

#[macro_use]
extern crate log;

use model::{Model, ID};

use abstutil::CmdArgs;
use geom::{Distance, Line, Polygon, Pt2D};
use map_gui::tools::CameraState;
use map_model::osm;
use map_model::raw::OriginalRoad;
use widgetry::{
    Canvas, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    SharedAppState, State, StyledButtons, Text, Toggle, Transition, VerticalAlignment, Widget,
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
    SetBoundaryPt1,
    SetBoundaryPt2(Pt2D),
}

impl MainState {
    fn new(ctx: &mut EventCtx) -> (App, MainState) {
        let mut args = CmdArgs::new();
        let load = args.optional_free();
        let include_bldgs = args.enabled("--bldgs");
        args.done();

        let model = if let Some(path) = load {
            Model::import(ctx, path, include_bldgs)
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
                        Toggle::switch(ctx, "intersection geometry", Key::G, false),
                        ctx.style()
                            .btn_outline_text("adjust boundary")
                            .build_def(ctx),
                        ctx.style()
                            .btn_solid_primary
                            .text("export to OSM")
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
                        } else if ctx.normal_left_click() {
                            return Transition::Push(edit::EditRoad::new(ctx, app, r));
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
                        let instructions = txt.draw(ctx);
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
                        let instructions = txt.draw(ctx);
                        self.panel.replace(ctx, "instructions", instructions);
                    }
                    None => {
                        match self.panel.event(ctx) {
                            Outcome::Clicked(x) => match x.as_ref() {
                                "close" => {
                                    return Transition::Pop;
                                }
                                "adjust boundary" => {
                                    self.mode = Mode::SetBoundaryPt1;
                                }
                                "export to OSM" => {
                                    app.model.export_to_osm();
                                }
                                _ => unreachable!(),
                            },
                            Outcome::Changed => {
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
                                let instructions = txt.draw(ctx);
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
                let instructions = txt.draw(ctx);
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
                let instructions = txt.draw(ctx);
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
        g.clear(Color::BLACK);

        // It's useful to see the origin.
        g.draw_polygon(Color::WHITE, Polygon::rectangle(100.0, 10.0));
        g.draw_polygon(Color::WHITE, Polygon::rectangle(10.0, 100.0));

        g.draw_polygon(
            Color::rgb(242, 239, 233),
            app.model.map.boundary_polygon.clone(),
        );
        app.model.world.draw(g, |_| true);

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

fn main() {
    widgetry::run(
        widgetry::Settings::new("RawMap editor").read_svg(Box::new(abstio::slurp_bytes)),
        |ctx| {
            let (app, state) = MainState::new(ctx);
            (app, vec![Box::new(state)])
        },
    );
}

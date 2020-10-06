use model::{Model, ID};

use abstutil::{CmdArgs, Timer};
use geom::{Distance, Line, Polygon};
use map_model::osm;
use map_model::raw::OriginalRoad;
use widgetry::{
    Btn, Canvas, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, ScreenPt, Text, VerticalAlignment, Widget, GUI,
};

mod model;
mod world;

struct UI {
    model: Model,
    state: State,
    panel: Panel,
    popup: Option<Drawable>,
    info_key_held: bool,

    last_id: Option<ID>,
}

enum State {
    Viewing,
    MovingIntersection(osm::NodeID),
    MovingBuilding(osm::OsmID),
    MovingRoadPoint(OriginalRoad, usize),
    CreatingRoad(osm::NodeID),
    // bool is show_tooltip
    PreviewIntersection(Drawable, bool),
}

impl UI {
    fn new(ctx: &mut EventCtx) -> UI {
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
        if !model.map.name.is_empty() {
            ctx.canvas.load_camera_state(&model.map.name);
        }
        let bounds = model.map.gps_bounds.to_bounds();
        ctx.canvas.map_dims = (bounds.width(), bounds.height());
        UI {
            model,
            state: State::Viewing,
            panel: Panel::new(Widget::col(vec![
                Line("Map Editor").small_heading().draw(ctx),
                Text::new().draw(ctx).named("current info"),
                Widget::col(vec![
                    Btn::text_fg("quit").build_def(ctx, Key::Escape),
                    Btn::text_fg("save raw map").build_def(ctx, None),
                    Btn::text_fg("preview all intersections").build_def(ctx, Key::G),
                ]),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            popup: None,
            info_key_held: false,

            last_id: None,
        }
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) {
        if self.info_key_held {
            self.info_key_held = !ctx.input.key_released(Key::LeftAlt);
        } else {
            self.info_key_held = ctx.input.pressed(Key::LeftAlt);
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            self.model.world.handle_mouseover(ctx);
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

        match self.state {
            State::Viewing => {
                {
                    let before = match self.last_id {
                        Some(ID::Road(r)) | Some(ID::RoadPoint(r, _)) => Some(r),
                        _ => None,
                    };
                    let after = match self.model.world.get_selection() {
                        Some(ID::Road(r)) | Some(ID::RoadPoint(r, _)) => Some(r),
                        _ => None,
                    };
                    if before != after {
                        if let Some(id) = before {
                            self.model.stop_showing_pts(id);
                        }
                        if let Some(r) = after {
                            self.model.show_r_points(r, ctx);
                            self.model.world.handle_mouseover(ctx);
                        }
                    }
                }

                match self.model.world.get_selection() {
                    Some(ID::Intersection(i)) => {
                        if ctx.input.pressed(Key::LeftControl) {
                            self.state = State::MovingIntersection(i);
                        } else if ctx.input.pressed(Key::R) {
                            self.state = State::CreatingRoad(i);
                        } else if ctx.input.pressed(Key::Backspace) {
                            self.model.delete_i(i);
                            self.model.world.handle_mouseover(ctx);
                        } else if !self.model.intersection_geom && ctx.input.pressed(Key::P) {
                            let draw = preview_intersection(i, &self.model, ctx);
                            self.state = State::PreviewIntersection(draw, false);
                        }
                    }
                    Some(ID::Building(b)) => {
                        if ctx.input.pressed(Key::LeftControl) {
                            self.state = State::MovingBuilding(b);
                        } else if ctx.input.pressed(Key::Backspace) {
                            self.model.delete_b(b);
                            self.model.world.handle_mouseover(ctx);
                        }
                    }
                    Some(ID::Road(r)) => {
                        if ctx.input.pressed(Key::Backspace) {
                            self.model.delete_r(r);
                            self.model.world.handle_mouseover(ctx);
                        } else if cursor.is_some() && ctx.input.pressed(Key::P) {
                            if let Some(id) = self.model.insert_r_pt(r, cursor.unwrap(), ctx) {
                                self.model.world.force_set_selection(id);
                            }
                        } else if ctx.input.pressed(Key::X) {
                            self.model.clear_r_pts(r, ctx);
                        }
                    }
                    Some(ID::RoadPoint(r, idx)) => {
                        if ctx.input.pressed(Key::LeftControl) {
                            self.state = State::MovingRoadPoint(r, idx);
                        } else if ctx.input.pressed(Key::Backspace) {
                            self.model.delete_r_pt(r, idx, ctx);
                            self.model.world.handle_mouseover(ctx);
                        }
                    }
                    None => {
                        match self.panel.event(ctx) {
                            Outcome::Clicked(x) => match x.as_ref() {
                                "quit" => {
                                    self.before_quit(ctx.canvas);
                                    std::process::exit(0);
                                }
                                "save raw map" => {
                                    // TODO Only do this for synthetic maps
                                    self.model.export();
                                }
                                "preview all intersections" => {
                                    if !self.model.intersection_geom {
                                        let draw = preview_all_intersections(&self.model, ctx);
                                        self.state = State::PreviewIntersection(draw, false);
                                    }
                                }
                                _ => unreachable!(),
                            },
                            _ => {
                                if ctx.input.pressed(Key::I) {
                                    if let Some(pt) = cursor {
                                        self.model.create_i(pt, ctx);
                                        self.model.world.handle_mouseover(ctx);
                                    }
                                // TODO Silly bug: Mouseover doesn't actually work! I think the
                                // cursor being dead-center messes
                                // up the precomputed triangles.
                                } else if ctx.input.pressed(Key::B) {
                                    if let Some(pt) = cursor {
                                        let id = self.model.create_b(pt, ctx);
                                        self.model.world.force_set_selection(id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            State::MovingIntersection(id) => {
                if let Some(pt) = cursor {
                    self.model.move_i(id, pt, ctx);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.state = State::Viewing;
                    }
                }
            }
            State::MovingBuilding(id) => {
                if let Some(pt) = cursor {
                    self.model.move_b(id, pt, ctx);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.state = State::Viewing;
                    }
                }
            }
            State::MovingRoadPoint(r, idx) => {
                if let Some(pt) = cursor {
                    self.model.move_r_pt(r, idx, pt, ctx);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.state = State::Viewing;
                    }
                }
            }
            State::CreatingRoad(i1) => {
                if ctx.input.pressed(Key::Escape) {
                    self.state = State::Viewing;
                    self.model.world.handle_mouseover(ctx);
                } else if let Some(ID::Intersection(i2)) = self.model.world.get_selection() {
                    if i1 != i2 && ctx.input.pressed(Key::R) {
                        self.model.create_r(i1, i2, ctx);
                        self.state = State::Viewing;
                        self.model.world.handle_mouseover(ctx);
                    }
                }
            }
            State::PreviewIntersection(_, ref mut show_tooltip) => {
                if *show_tooltip && ctx.input.key_released(Key::RightAlt) {
                    *show_tooltip = false;
                } else if !*show_tooltip && ctx.input.pressed(Key::RightAlt) {
                    *show_tooltip = true;
                }

                // TODO Woops, not communicating this kind of thing anymore
                if ctx.input.pressed(Key::P) {
                    self.state = State::Viewing;
                    self.model.world.handle_mouseover(ctx);
                }
            }
        }

        self.popup = None;
        if self.info_key_held {
            if let Some(id) = self.model.world.get_selection() {
                let txt = self.model.describe_obj(id);
                // TODO We used to display actions and hotkeys here
                self.popup = Some(ctx.upload(txt.render_to_batch(ctx.prerender)));
            }
        }

        self.last_id = self.model.world.get_selection();
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::BLACK);

        // It's useful to see the origin.
        g.draw_polygon(Color::WHITE, Polygon::rectangle(100.0, 10.0));
        g.draw_polygon(Color::WHITE, Polygon::rectangle(10.0, 100.0));

        g.draw_polygon(
            Color::rgb(242, 239, 233),
            self.model.map.boundary_polygon.clone(),
        );
        match self.state {
            State::PreviewIntersection(_, _) => self.model.world.draw(g, |id| match id {
                ID::Intersection(_) => false,
                _ => true,
            }),
            _ => self.model.world.draw(g, |_| true),
        }

        match self.state {
            State::CreatingRoad(i1) => {
                if let Some(cursor) = g.get_cursor_in_map_space() {
                    if let Some(l) = Line::new(self.model.map.intersections[&i1].point, cursor) {
                        g.draw_polygon(Color::GREEN, l.make_polygons(Distance::meters(5.0)));
                    }
                }
            }
            State::Viewing
            | State::MovingIntersection(_)
            | State::MovingBuilding(_)
            | State::MovingRoadPoint(_, _) => {}
            State::PreviewIntersection(ref draw, show_tooltip) => {
                g.redraw(draw);

                if show_tooltip {
                    // TODO Argh, covers up mouseover tooltip.
                    if let Some(cursor) = g.canvas.get_cursor_in_map_space() {
                        g.draw_mouse_tooltip(Text::from(Line(cursor.to_string())));
                    }
                }
            }
        };

        self.panel.draw(g);
        if let Some(ref popup) = self.popup {
            g.redraw_at(ScreenPt::new(0.0, 0.0), popup);
        }
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        if !self.model.map.name.is_empty() {
            canvas.save_camera_state(&self.model.map.name);
        }
    }

    fn before_quit(&self, canvas: &Canvas) {
        if !self.model.map.name.is_empty() {
            canvas.save_camera_state(&self.model.map.name);
        }
    }
}

fn preview_intersection(i: osm::NodeID, model: &Model, ctx: &EventCtx) -> Drawable {
    let (intersection, roads, debug) = model
        .map
        .preview_intersection(i, &mut Timer::new("calculate intersection_polygon"));
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
                .render_to_batch(ctx.prerender)
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
        let (intersection, _, _) = model.map.preview_intersection(*i, &mut timer);
        batch.push(Color::ORANGE.alpha(0.5), intersection);
    }
    batch.upload(ctx)
}

fn main() {
    widgetry::run(widgetry::Settings::new("Synthetic map editor"), |ctx| {
        UI::new(ctx)
    });
}

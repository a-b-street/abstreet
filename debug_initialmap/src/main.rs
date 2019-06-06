use abstutil::{find_next_file, find_prev_file, read_binary, Timer};
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, GUI};
use geom::{Distance, Polygon};
use map_model::raw_data;
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use std::collections::HashSet;
use std::{env, process};
use viewer::World;

struct UI {
    world: World<ID>,
    filename: String,
    // TODO Or, if these are common things, the World could also hold this state.
    selected: Option<ID>,
    hide: HashSet<ID>,
    osd: Text,
}

impl UI {
    fn new(filename: &str, world: World<ID>) -> UI {
        UI {
            world,
            filename: filename.to_string(),
            selected: None,
            hide: HashSet::new(),
            osd: Text::new(),
        }
    }

    fn load_different(&mut self, filename: String, ctx: &mut EventCtx) {
        self.world = load_initial_map(&filename, ctx);
        self.selected = None;
        self.filename = filename;
        self.hide.clear();
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);

        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            self.selected = self.world.mouseover_something(ctx, &self.hide);
        }

        if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }

        if let Some(prev) = find_prev_file(self.filename.clone()) {
            if ctx.input.key_pressed(Key::Comma, "load previous map") {
                self.load_different(prev, ctx);
            }
        }
        if let Some(next) = find_next_file(self.filename.clone()) {
            if ctx.input.key_pressed(Key::Dot, "load next map") {
                self.load_different(next, ctx);
            }
        }

        if let Some(id) = self.selected {
            if ctx.input.key_pressed(Key::H, "hide this") {
                self.hide.insert(id);
                self.selected = None;
            }
        }
        if !self.hide.is_empty() {
            if ctx.input.key_pressed(Key::K, "unhide everything") {
                self.hide.clear();
            }
        }

        self.osd = Text::new();
        ctx.input.populate_osd(&mut self.osd);
        EventLoopMode::InputOnly
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::WHITE);

        self.world.draw(g, &self.hide);

        if let Some(id) = self.selected {
            self.world.draw_selected(g, id);
        }

        g.draw_blocking_text(&self.osd, ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run("InitialMap debugger", 1024.0, 768.0, |ctx| {
        ctx.canvas.cam_zoom = 4.0;
        UI::new(&args[1], load_initial_map(&args[1], ctx))
    });
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum ID {
    // Forwards?
    HalfRoad(StableRoadID, bool),
    Intersection(StableIntersectionID),
}

impl viewer::ObjectID for ID {
    fn zorder(&self) -> usize {
        match self {
            ID::HalfRoad(_, _) => 0,
            ID::Intersection(_) => 1,
        }
    }
}

fn load_initial_map(filename: &str, ctx: &mut EventCtx) -> World<ID> {
    let data: raw_data::InitialMap =
        read_binary(filename, &mut Timer::new("load InitialMap")).unwrap();

    let mut w = World::new(&data.bounds);

    for r in data.roads.values() {
        if r.fwd_width > Distance::ZERO {
            w.add_obj(
                ctx.prerender,
                ID::HalfRoad(r.id, true),
                r.trimmed_center_pts
                    .shift_right(r.fwd_width / 2.0)
                    .unwrap()
                    .make_polygons(r.fwd_width),
                Color::grey(0.8),
                Text::from_line(format!(
                    "{} forwards, {} long",
                    r.id,
                    r.trimmed_center_pts.length()
                )),
            );
        }
        if r.back_width > Distance::ZERO {
            w.add_obj(
                ctx.prerender,
                ID::HalfRoad(r.id, false),
                r.trimmed_center_pts
                    .shift_left(r.back_width / 2.0)
                    .unwrap()
                    .make_polygons(r.back_width),
                Color::grey(0.6),
                Text::from_line(format!(
                    "{} backwards, {} long",
                    r.id,
                    r.trimmed_center_pts.length()
                )),
            );
        }
    }

    for i in data.intersections.values() {
        w.add_obj(
            ctx.prerender,
            ID::Intersection(i.id),
            Polygon::new(&i.polygon),
            Color::RED,
            Text::from_line(format!("{}", i.id)),
        );
    }

    if let Some(id) = data.focus_on {
        ctx.canvas
            .center_on_map_pt(w.get_center(ID::Intersection(id)));
    }

    w
}

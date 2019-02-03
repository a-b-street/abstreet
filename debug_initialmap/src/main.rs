use abstutil::{find_next_file, find_prev_file, read_binary, Timer};
use ezgui::{Canvas, Color, EventCtx, EventLoopMode, GfxCtx, Key, Prerender, Text, GUI};
use geom::{Distance, Polygon};
use map_model::raw_data;
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use std::{env, process};
use viewer::World;

struct UI {
    world: World<ID>,
    filename: String,
    selected: Option<ID>,
}

impl UI {
    fn new(filename: &str, world: World<ID>) -> UI {
        UI {
            world,
            filename: filename.to_string(),
            selected: None,
        }
    }
}

impl GUI<Text> for UI {
    fn event(&mut self, ctx: EventCtx) -> (EventLoopMode, Text) {
        ctx.canvas.handle_event(ctx.input);

        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            self.selected = self.world.mouseover_something(&ctx);
        }

        if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }

        if let Some(prev) = find_prev_file(&self.filename) {
            if ctx.input.key_pressed(Key::Comma, "load previous map") {
                self.world = load_initial_map(&prev, ctx.canvas, ctx.prerender);
                self.selected = None;
                self.filename = prev;
            }
        }
        if let Some(next) = find_next_file(&self.filename) {
            if ctx.input.key_pressed(Key::Dot, "load next map") {
                self.world = load_initial_map(&next, ctx.canvas, ctx.prerender);
                self.selected = None;
                self.filename = next;
            }
        }

        let mut osd = Text::new();
        ctx.input.populate_osd(&mut osd);
        (EventLoopMode::InputOnly, osd)
    }

    fn draw(&self, g: &mut GfxCtx, osd: &Text) {
        g.clear(Color::WHITE);

        self.world.draw(g);

        if let Some(id) = self.selected {
            self.world.draw_selected(g, id);
        }

        g.draw_blocking_text(osd.clone(), ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run("InitialMap debugger", 1024.0, 768.0, |canvas, prerender| {
        canvas.cam_zoom = 4.0;
        UI::new(&args[1], load_initial_map(&args[1], canvas, prerender))
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

fn load_initial_map(filename: &str, canvas: &mut Canvas, prerender: &Prerender) -> World<ID> {
    let data: raw_data::InitialMap = read_binary(filename, &mut Timer::new("load data")).unwrap();

    let mut w = World::new(&data.bounds);

    for r in data.roads.values() {
        if r.fwd_width > Distance::ZERO {
            w.add_obj(
                prerender,
                ID::HalfRoad(r.id, true),
                r.trimmed_center_pts
                    .shift_right(r.fwd_width / 2.0)
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
                prerender,
                ID::HalfRoad(r.id, false),
                r.trimmed_center_pts
                    .shift_left(r.back_width / 2.0)
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
            prerender,
            ID::Intersection(i.id),
            Polygon::new(&i.polygon),
            Color::RED,
            Text::from_line(format!("{}", i.id)),
        );
    }

    if let Some(id) = data.focus_on {
        canvas.center_on_map_pt(w.get_center(ID::Intersection(id)));
    }

    w
}

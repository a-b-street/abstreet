use abstutil::Timer;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, GUI};
use geom::{Distance, Polygon};
use map_model::raw_data::{Hint, Hints, InitialMap, StableIntersectionID, StableRoadID};
use std::collections::HashSet;
use std::{env, process};
use viewer::World;

// Bit bigger than buses
const MIN_ROAD_LENGTH: Distance = Distance::const_meters(13.0);

struct UI {
    world: World<ID>,
    data: InitialMap,
    hints: Hints,
    // TODO Or, if these are common things, the World could also hold this state.
    selected: Option<ID>,
    hide: HashSet<ID>,
    osd: Text,
}

impl UI {
    fn new(filename: &str, ctx: &mut EventCtx) -> UI {
        let mut timer = Timer::new(&format!("load {}", filename));
        let raw: map_model::raw_data::Map = abstutil::read_binary(filename, &mut timer).unwrap();
        let map_name = abstutil::basename(filename);
        let gps_bounds = raw.get_gps_bounds();
        let mut data = InitialMap::new(
            map_name,
            &raw,
            &gps_bounds,
            &gps_bounds.to_bounds(),
            &mut timer,
        );
        let hints: Hints = abstutil::read_json(&format!("../data/hints/{}.json", data.name))
            .unwrap_or(Hints { hints: Vec::new() });
        data.apply_hints(&hints);

        let world = initial_map_to_world(&data, ctx);

        UI {
            world,
            data,
            hints,
            selected: None,
            hide: HashSet::new(),
            osd: Text::new(),
        }
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);

        if ctx.redo_mouseover() {
            self.selected = self.world.mouseover_something(ctx, &self.hide);
        }

        if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }
        if ctx
            .input
            .key_pressed(Key::S, &format!("save {} hints", self.hints.hints.len()))
        {
            abstutil::write_json(
                &format!("../data/hints/{}.json", self.data.name),
                &self.hints,
            )
            .unwrap();
        }

        if let Some(id) = self.selected {
            if ctx.input.key_pressed(Key::H, "hide this") {
                self.hide.insert(id);
                self.selected = None;
            }
        }
        if let Some(ID::HalfRoad(r, _)) = self.selected {
            if ctx.input.key_pressed(Key::M, "merge") {
                self.hints.hints.push(Hint::MergeRoad(r));
                self.data.merge_road(r);
                self.world = initial_map_to_world(&self.data, ctx);
                self.selected = None;
            } else if ctx.input.key_pressed(Key::D, "delete") {
                self.hints.hints.push(Hint::DeleteRoad(r));
                self.data.delete_road(r);
                self.world = initial_map_to_world(&self.data, ctx);
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
        UI::new(&args[1], ctx)
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

fn initial_map_to_world(data: &InitialMap, ctx: &mut EventCtx) -> World<ID> {
    let mut w = World::new(&data.bounds);

    for r in data.roads.values() {
        let len = r.trimmed_center_pts.length();
        if r.fwd_width > Distance::ZERO {
            w.add_obj(
                ctx.prerender,
                ID::HalfRoad(r.id, true),
                r.trimmed_center_pts
                    .shift_right(r.fwd_width / 2.0)
                    .unwrap()
                    .make_polygons(r.fwd_width),
                if len < MIN_ROAD_LENGTH {
                    Color::CYAN
                } else {
                    Color::grey(0.8)
                },
                Text::from_line(format!("{} forwards, {} long", r.id, len)),
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
                if len < MIN_ROAD_LENGTH {
                    Color::GREEN
                } else {
                    Color::grey(0.6)
                },
                Text::from_line(format!("{} backwards, {} long", r.id, len)),
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

    w
}

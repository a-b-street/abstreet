use abstutil::Timer;
use ezgui::{hotkey, Color, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text, GUI};
use geom::{Distance, Polygon};
use map_model::raw_data::{Hint, Hints, InitialMap, Map, StableIntersectionID, StableRoadID};
use std::collections::HashSet;
use std::{env, process};
use viewer::World;

// Bit bigger than buses
const MIN_ROAD_LENGTH: Distance = Distance::const_meters(13.0);

struct UI {
    menu: ModalMenu,
    world: World<ID>,
    data: InitialMap,
    raw: Map,
    hints: Hints,
    // TODO Or, if these are common things, the World could also hold this state.
    selected: Option<ID>,
    osd: Text,
}

impl UI {
    fn new(filename: &str, ctx: &mut EventCtx) -> UI {
        ctx.loading_screen(&format!("load {}", filename), |ctx, mut timer| {
            let raw: Map = abstutil::read_binary(filename, &mut timer).unwrap();
            let map_name = abstutil::basename(filename);
            let gps_bounds = &raw.gps_bounds;
            let mut data = InitialMap::new(
                map_name,
                &raw,
                gps_bounds,
                &gps_bounds.to_bounds(),
                &mut timer,
            );
            let hints = Hints::load();
            data.apply_hints(&hints, &raw, &mut timer);

            let world = initial_map_to_world(&data, ctx);

            UI {
                menu: ModalMenu::new(
                    "Fix Map Geometry",
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (hotkey(Key::S), "save"),
                        (hotkey(Key::R), "reset hints"),
                        (hotkey(Key::U), "undo last hint"),
                    ],
                    ctx,
                ),
                world,
                raw,
                data,
                hints,
                selected: None,
                osd: Text::new(),
            }
        })
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        {
            let len = self.hints.hints.len();
            let mut txt = Text::prompt("Fix Map Geometry");
            txt.push(format!("[cyan:{}] hints", len));
            for i in (1..=5).rev() {
                if len >= i {
                    txt.add_line(match self.hints.hints[len - i] {
                        Hint::MergeRoad(_) => "MergeRoad(...)".to_string(),
                        Hint::DeleteRoad(_) => "DeleteRoad(...)".to_string(),
                        Hint::MergeDegenerateIntersection(_) => {
                            "MergeDegenerateIntersection(...)".to_string()
                        }
                    });
                } else {
                    txt.add_line("...".to_string());
                }
            }
            if let Some(ID::Road(r)) = self.selected {
                txt.push(format!(
                    "[red:{}] is {} long",
                    r,
                    self.data.roads[&r].trimmed_center_pts.length()
                ));
                for (k, v) in &self.raw.roads[&r].osm_tags {
                    txt.push(format!("[cyan:{}] = [red:{}]", k, v));
                }
            }
            if let Some(ID::Intersection(i)) = self.selected {
                txt.push(format!("[red:{}] OSM tag diffs:", i));
                let roads = &self.data.intersections[&i].roads;
                if roads.len() == 2 {
                    let mut iter = roads.iter();
                    let r1_tags = &self.raw.roads[iter.next().unwrap()].osm_tags;
                    let r2_tags = &self.raw.roads[iter.next().unwrap()].osm_tags;

                    for (k, v1) in r1_tags {
                        if let Some(v2) = r2_tags.get(k) {
                            if v1 != v2 {
                                txt.push(format!("[cyan:{}] = [red:{}] / [red:{}]", k, v1, v2));
                            }
                        } else {
                            txt.push(format!("[cyan:{}] = [red:{}] / MISSING", k, v1));
                        }
                    }
                    for (k, v2) in r2_tags {
                        if !r1_tags.contains_key(k) {
                            txt.push(format!("[cyan:{}] = MISSING / [red:{}] ", k, v2));
                        }
                    }
                }
            }
            self.menu.handle_event(ctx, Some(txt));
        }
        ctx.canvas.handle_event(ctx.input);

        if ctx.redo_mouseover() {
            self.selected = self.world.mouseover_something(ctx, &HashSet::new());
        }

        if self.menu.action("quit") {
            process::exit(0);
        }
        if !self.hints.hints.is_empty() {
            if self.menu.action("save") {
                abstutil::write_json("../data/hints.json", &self.hints).unwrap();
                println!("Saved hints.json");
            }

            let recalc = if self.menu.action("undo last hint") {
                self.hints.hints.pop();
                true
            } else if self.menu.action("reset hints") {
                self.hints.hints.clear();
                true
            } else {
                false
            };
            if recalc {
                ctx.loading_screen("recalculate map from hints", |ctx, mut timer| {
                    let gps_bounds = &self.raw.gps_bounds;
                    self.data = InitialMap::new(
                        self.data.name.clone(),
                        &self.raw,
                        gps_bounds,
                        &gps_bounds.to_bounds(),
                        &mut timer,
                    );
                    self.data.apply_hints(&self.hints, &self.raw, &mut timer);
                    self.world = initial_map_to_world(&self.data, ctx);
                    self.selected = None;
                });
            }
        }

        if let Some(ID::Road(r)) = self.selected {
            if ctx.input.key_pressed(Key::M, "merge") {
                self.hints
                    .hints
                    .push(Hint::MergeRoad(self.raw.roads[&r].orig_id()));
                self.data.merge_road(r, &mut Timer::new("merge road"));
                self.world = initial_map_to_world(&self.data, ctx);
                self.selected = None;
            } else if ctx.input.key_pressed(Key::D, "delete") {
                self.hints
                    .hints
                    .push(Hint::DeleteRoad(self.raw.roads[&r].orig_id()));
                self.data.delete_road(r, &mut Timer::new("delete road"));
                self.world = initial_map_to_world(&self.data, ctx);
                self.selected = None;
            }
        }
        if let Some(ID::Intersection(i)) = self.selected {
            if self.data.intersections[&i].roads.len() == 2
                && ctx.input.key_pressed(Key::M, "merge")
            {
                self.hints.hints.push(Hint::MergeDegenerateIntersection(
                    self.raw.intersections[&i].orig_id(),
                ));
                self.data
                    .merge_degenerate_intersection(i, &mut Timer::new("merge intersection"));
                self.world = initial_map_to_world(&self.data, ctx);
                self.selected = None;
            }
        }

        self.osd = Text::new();
        ctx.input.populate_osd(&mut self.osd);
        EventLoopMode::InputOnly
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::WHITE);

        self.world.draw(g, &HashSet::new());

        if let Some(id) = self.selected {
            self.world.draw_selected(g, id);
        }

        self.menu.draw(g);
        g.draw_blocking_text(&self.osd, ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run("InitialMap debugger", 1800.0, 800.0, |ctx| {
        ctx.canvas.cam_zoom = 4.0;
        UI::new(&args[1], ctx)
    });
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum ID {
    Road(StableRoadID),
    Intersection(StableIntersectionID),
}

impl viewer::ObjectID for ID {
    fn zorder(&self) -> usize {
        match self {
            ID::Road(_) => 0,
            ID::Intersection(_) => 1,
        }
    }
}

fn initial_map_to_world(data: &InitialMap, ctx: &mut EventCtx) -> World<ID> {
    let mut w = World::new(&data.bounds);

    for r in data.roads.values() {
        w.add_obj(
            ctx.prerender,
            ID::Road(r.id),
            (if r.fwd_width >= r.back_width {
                r.trimmed_center_pts
                    .shift_right((r.fwd_width - r.back_width) / 2.0)
            } else {
                r.trimmed_center_pts
                    .shift_left((r.back_width - r.fwd_width) / 2.0)
            })
            .unwrap()
            .make_polygons(r.fwd_width + r.back_width),
            if r.trimmed_center_pts.length() < MIN_ROAD_LENGTH {
                Color::CYAN
            } else {
                Color::grey(0.8)
            },
            Text::from_line(r.id.to_string()),
        );
    }

    for i in data.intersections.values() {
        w.add_obj(
            ctx.prerender,
            ID::Intersection(i.id),
            Polygon::new(&i.polygon),
            if i.roads.len() == 2 {
                Color::RED
            } else {
                Color::BLACK
            },
            Text::from_line(format!("{}", i.id)),
        );
    }

    w
}

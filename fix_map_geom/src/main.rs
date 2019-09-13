use abstutil::Timer;
use ezgui::world::{Object, ObjectID, World};
use ezgui::{
    hotkey, Color, EventCtx, EventLoopMode, GfxCtx, Key, Line, ModalMenu, Text, WarpingItemSlider,
    GUI,
};
use geom::{Circle, Distance, PolyLine, Polygon};
use map_model::raw_data::{Hint, Hints, InitialMap, Map, StableIntersectionID, StableRoadID};
use map_model::LANE_THICKNESS;
use std::{env, process};

// Bit bigger than buses
const MIN_ROAD_LENGTH: Distance = Distance::const_meters(13.0);

struct UI {
    world: World<ID>,
    data: InitialMap,
    raw: Map,
    hints: Hints,
    state: State,
}

enum State {
    Main { menu: ModalMenu, osd: Text },
    BrowsingHints(WarpingItemSlider<Hint>),
    BanTurnsBetween { from: StableRoadID, osd: Text },
    MovingIntersection(StableIntersectionID, Text),
}

impl State {
    fn main(ctx: &mut EventCtx) -> State {
        State::Main {
            menu: ModalMenu::new(
                "Fix Map Geometry",
                vec![vec![
                    (hotkey(Key::Escape), "quit"),
                    (hotkey(Key::S), "save"),
                    (hotkey(Key::R), "reset hints"),
                    (hotkey(Key::U), "undo last hint"),
                    (hotkey(Key::B), "browse hints"),
                ]],
                ctx,
            ),
            osd: Text::new(),
        }
    }
}

impl UI {
    fn new(filename: &str, ctx: &mut EventCtx) -> UI {
        ctx.loading_screen(&format!("load {}", filename), |ctx, mut timer| {
            let raw: Map = abstutil::read_binary(filename, &mut timer).unwrap();
            let map_name = abstutil::basename(filename);
            let mut data = InitialMap::new(map_name, &raw, &raw.gps_bounds.to_bounds(), &mut timer);
            let hints = Hints::load();
            data.apply_hints(&hints, &raw, &mut timer);

            let world = initial_map_to_world(&data, ctx);

            UI {
                world,
                data,
                raw,
                hints,
                state: State::main(ctx),
            }
        })
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        match self.state {
            State::Main {
                ref mut menu,
                ref mut osd,
            } => {
                {
                    if ctx.redo_mouseover() {
                        self.world.handle_mouseover(ctx);
                    }

                    let len = self.hints.hints.len();
                    let mut txt = Text::prompt("Fix Map Geometry");
                    txt.add_appended(vec![
                        Line(len.to_string()).fg(Color::CYAN),
                        Line(" hints, "),
                        Line(self.hints.parking_overrides.len().to_string()).fg(Color::CYAN),
                        Line(" parking overrides"),
                    ]);
                    if let Some(ID::Road(r)) = self.world.get_selection() {
                        txt.add_appended(vec![
                            Line(r.to_string()).fg(Color::RED),
                            Line(format!(
                                " is {} long",
                                self.data.roads[&r].trimmed_center_pts.length()
                            )),
                        ]);
                        if self.data.roads[&r].has_parking() {
                            txt.add(Line("Has parking"));
                        } else {
                            txt.add(Line("No parking"));
                        }
                        for (k, v) in &self.raw.roads[&r].osm_tags {
                            txt.add_appended(vec![
                                Line(k).fg(Color::RED),
                                Line(" = "),
                                Line(v).fg(Color::CYAN),
                            ]);
                        }
                    }
                    if let Some(ID::Intersection(i)) = self.world.get_selection() {
                        txt.add_appended(vec![
                            Line(i.to_string()).fg(Color::RED),
                            Line(" OSM tag diffs:"),
                        ]);
                        let roads = &self.data.intersections[&i].roads;
                        if roads.len() == 2 {
                            let mut iter = roads.iter();
                            let r1_tags = &self.raw.roads[iter.next().unwrap()].osm_tags;
                            let r2_tags = &self.raw.roads[iter.next().unwrap()].osm_tags;

                            for (k, v1) in r1_tags {
                                if let Some(v2) = r2_tags.get(k) {
                                    if v1 != v2 {
                                        txt.add_appended(vec![
                                            Line(k).fg(Color::RED),
                                            Line(" = "),
                                            Line(v1).fg(Color::CYAN),
                                            Line(" / "),
                                            Line(v2).fg(Color::CYAN),
                                        ]);
                                    }
                                } else {
                                    txt.add_appended(vec![
                                        Line(k).fg(Color::RED),
                                        Line(" = "),
                                        Line(v1).fg(Color::CYAN),
                                        Line(" / "),
                                        Line("MISSING").fg(Color::CYAN),
                                    ]);
                                }
                            }
                            for (k, v2) in r2_tags {
                                if !r1_tags.contains_key(k) {
                                    txt.add_appended(vec![
                                        Line(k).fg(Color::RED),
                                        Line(" = "),
                                        Line("MISSING").fg(Color::CYAN),
                                        Line(" / "),
                                        Line(v2).fg(Color::CYAN),
                                    ]);
                                }
                            }
                        }
                    }
                    menu.handle_event(ctx, Some(txt));
                }
                ctx.canvas.handle_event(ctx.input);

                if menu.action("quit") {
                    process::exit(0);
                }
                if !self.hints.hints.is_empty() {
                    if menu.action("save") {
                        abstutil::write_json("../data/hints.json", &self.hints).unwrap();
                        println!("Saved hints.json");
                    }

                    if menu.action("browse hints") {
                        self.state = State::BrowsingHints(WarpingItemSlider::new(
                            // TODO bleh
                            self.hints
                                .hints
                                .iter()
                                .filter_map(|h| {
                                    let pt = match h {
                                        Hint::MergeRoad(r)
                                        | Hint::DeleteRoad(r)
                                        | Hint::BanTurnsBetween(r, _) => {
                                            self.raw.roads[&self.raw.find_r(*r)?].center_points[0]
                                        }
                                        Hint::MergeDegenerateIntersection(i) => {
                                            self.raw.intersections[&self.raw.find_i(*i)?].point
                                        }
                                    };
                                    Some((pt, h.clone(), Text::from(Line(describe(h)))))
                                })
                                .collect(),
                            "Hints Browser",
                            "hint",
                            ctx,
                        ));
                        return EventLoopMode::InputOnly;
                    }

                    let recalc = if menu.action("undo last hint") {
                        self.hints.hints.pop();
                        true
                    } else if menu.action("reset hints") {
                        self.hints.hints.clear();
                        self.hints.parking_overrides.clear();
                        true
                    } else {
                        false
                    };
                    if recalc {
                        ctx.loading_screen("recalculate map from hints", |ctx, mut timer| {
                            self.data = InitialMap::new(
                                self.data.name.clone(),
                                &self.raw,
                                &self.raw.gps_bounds.to_bounds(),
                                &mut timer,
                            );
                            self.data.apply_hints(&self.hints, &self.raw, &mut timer);
                            self.world = initial_map_to_world(&self.data, ctx);
                        });
                        return EventLoopMode::InputOnly;
                    }
                }

                if let Some(ID::Road(r)) = self.world.get_selection() {
                    if ctx.input.key_pressed(Key::M, "merge") {
                        self.hints
                            .hints
                            .push(Hint::MergeRoad(self.raw.roads[&r].orig_id));
                        self.data.merge_road(r, &mut Timer::new("merge road"));
                        self.world = initial_map_to_world(&self.data, ctx);
                    } else if ctx.input.key_pressed(Key::D, "delete") {
                        self.hints
                            .hints
                            .push(Hint::DeleteRoad(self.raw.roads[&r].orig_id));
                        self.data.delete_road(r, &mut Timer::new("delete road"));
                        self.world = initial_map_to_world(&self.data, ctx);
                    } else if ctx.input.key_pressed(Key::P, "toggle parking") {
                        let has_parking = !self.data.roads[&r].has_parking();
                        self.hints
                            .parking_overrides
                            .insert(self.raw.roads[&r].orig_id, has_parking);
                        self.data.override_parking(
                            r,
                            has_parking,
                            &mut Timer::new("override parking"),
                        );
                        self.world = initial_map_to_world(&self.data, ctx);
                    } else if ctx
                        .input
                        .key_pressed(Key::T, "ban turns between this road and another")
                    {
                        self.state = State::BanTurnsBetween {
                            from: r,
                            osd: Text::new(),
                        };
                        return EventLoopMode::InputOnly;
                    } else if ctx.input.key_pressed(Key::E, "examine") {
                        let road = &self.data.roads[&r];
                        println!("{} between {} and {}", road.id, road.src_i, road.dst_i);
                        println!("Orig pts: {}", road.original_center_pts);
                        println!("Trimmed pts: {}", road.trimmed_center_pts);
                    }
                }
                if let Some(ID::Intersection(i)) = self.world.get_selection() {
                    if ctx.input.key_pressed(Key::LeftControl, "move intersection") {
                        self.state = State::MovingIntersection(i, Text::new());
                        return EventLoopMode::InputOnly;
                    }
                    if ctx.input.key_pressed(Key::E, "examine") {
                        let intersection = &self.data.intersections[&i];
                        println!("{} has roads: {:?}", intersection.id, intersection.roads);
                        println!("Points: {:?}", intersection.polygon);
                    }
                    if self.data.intersections[&i].roads.len() == 2
                        && ctx.input.key_pressed(Key::M, "merge")
                    {
                        self.hints.hints.push(Hint::MergeDegenerateIntersection(
                            self.raw.intersections[&i].orig_id,
                        ));
                        self.data.merge_degenerate_intersection(
                            i,
                            &mut Timer::new("merge intersection"),
                        );
                        self.world = initial_map_to_world(&self.data, ctx);
                    }
                }

                *osd = Text::new();
                ctx.input.populate_osd(osd);
                EventLoopMode::InputOnly
            }
            State::BrowsingHints(ref mut slider) => {
                ctx.canvas.handle_event(ctx.input);
                if let Some((evmode, _)) = slider.event(ctx) {
                    evmode
                } else {
                    self.state = State::main(ctx);
                    EventLoopMode::InputOnly
                }
            }
            State::BanTurnsBetween { from, ref mut osd } => {
                ctx.canvas.handle_event(ctx.input);
                if ctx.redo_mouseover() {
                    self.world.handle_mouseover(ctx);
                }

                if ctx.input.key_pressed(Key::Escape, "cancel") {
                    self.state = State::main(ctx);
                    return EventLoopMode::InputOnly;
                } else if let Some(ID::Road(r)) = self.world.get_selection() {
                    // TODO Why do we use data and not raw here?
                    let (i1, i2) = (self.data.roads[&from].src_i, self.data.roads[&from].dst_i);
                    let (i3, i4) = (self.data.roads[&r].src_i, self.data.roads[&r].dst_i);

                    if from != r
                        && (i1 == i3 || i1 == i4 || i2 == i3 || i2 == i4)
                        && ctx.input.key_pressed(Key::T, "ban turns to this road")
                    {
                        self.hints.hints.push(Hint::BanTurnsBetween(
                            self.raw.roads[&from].orig_id,
                            self.raw.roads[&r].orig_id,
                        ));
                        // There's nothing to change about our model here.
                        self.state = State::main(ctx);
                        return EventLoopMode::InputOnly;
                    }
                }

                *osd = Text::new();
                ctx.input.populate_osd(osd);
                EventLoopMode::InputOnly
            }
            State::MovingIntersection(i, ref mut osd) => {
                ctx.canvas.handle_event(ctx.input);

                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if ctx
                        .input
                        .key_pressed(Key::LeftControl, "move intersection here")
                    {
                        // TODO Record a hint... but have to overwrite
                        self.data
                            .move_intersection(i, pt, &mut Timer::new("move intersection"));
                        self.world = initial_map_to_world(&self.data, ctx);
                    }
                }

                if ctx
                    .input
                    .key_pressed(Key::Escape, "stop moving intersection")
                {
                    self.state = State::main(ctx);
                    return EventLoopMode::InputOnly;
                }
                *osd = Text::new();
                ctx.input.populate_osd(osd);
                EventLoopMode::InputOnly
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.clear(Color::WHITE);

        self.world.draw(g);

        match self.state {
            State::Main { ref menu, ref osd } => {
                menu.draw(g);
                g.draw_blocking_text(osd, ezgui::BOTTOM_LEFT);
            }
            State::BrowsingHints(ref slider) => {
                let poly = match slider.get().1 {
                    Hint::MergeRoad(r) | Hint::DeleteRoad(r) | Hint::BanTurnsBetween(r, _) => {
                        PolyLine::new(
                            self.raw.roads[&self.raw.find_r(*r).unwrap()]
                                .center_points
                                .clone(),
                        )
                        // Just make up a width
                        .make_polygons(4.0 * LANE_THICKNESS)
                    }
                    Hint::MergeDegenerateIntersection(i) => Circle::new(
                        self.raw.intersections[&self.raw.find_i(*i).unwrap()].point,
                        Distance::meters(10.0),
                    )
                    .to_polygon(),
                };
                g.draw_polygon(Color::PURPLE.alpha(0.7), &poly);

                slider.draw(g);
            }
            State::BanTurnsBetween { ref osd, .. } => {
                g.draw_blocking_text(osd, ezgui::BOTTOM_LEFT);
            }
            State::MovingIntersection(_, ref osd) => {
                g.draw_blocking_text(osd, ezgui::BOTTOM_LEFT);
            }
        }
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

impl ObjectID for ID {
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
        w.add(
            ctx.prerender,
            Object::new(
                ID::Road(r.id),
                if r.trimmed_center_pts.length() < MIN_ROAD_LENGTH {
                    Color::CYAN
                } else if r.has_parking() {
                    Color::grey(0.5)
                } else {
                    Color::grey(0.8)
                },
                (if r.fwd_width >= r.back_width {
                    r.trimmed_center_pts
                        .shift_right((r.fwd_width - r.back_width) / 2.0)
                } else {
                    r.trimmed_center_pts
                        .shift_left((r.back_width - r.fwd_width) / 2.0)
                })
                .unwrap()
                .make_polygons(r.fwd_width + r.back_width),
            )
            .tooltip(Text::from(Line(r.id.to_string()))),
        );
    }

    for i in data.intersections.values() {
        w.add(
            ctx.prerender,
            Object::new(
                ID::Intersection(i.id),
                if i.roads.len() == 2 {
                    Color::RED
                } else {
                    Color::BLACK
                },
                Polygon::new(&i.polygon),
            )
            .tooltip(Text::from(Line(i.id.to_string()))),
        );
    }

    w
}

fn describe(hint: &Hint) -> String {
    match hint {
        Hint::MergeRoad(_) => "MergeRoad(...)".to_string(),
        Hint::DeleteRoad(_) => "DeleteRoad(...)".to_string(),
        Hint::MergeDegenerateIntersection(_) => "MergeDegenerateIntersection(...)".to_string(),
        Hint::BanTurnsBetween(_, _) => "BanTurnsBetween(...)".to_string(),
    }
}

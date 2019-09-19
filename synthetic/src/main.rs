mod model;

use abstutil::CmdArgs;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Line, Text, Wizard, GUI};
use geom::{Distance, Line, Polygon, Pt2D};
use map_model::raw_data::{StableBuildingID, StableIntersectionID, StableRoadID};
use model::{Direction, Model, ID};
use std::process;

struct UI {
    model: Model,
    state: State,
    osd: Text,
}

enum State {
    Viewing,
    MovingIntersection(StableIntersectionID),
    MovingBuilding(StableBuildingID),
    MovingRoadPoint(StableRoadID, usize),
    LabelingBuilding(StableBuildingID, Wizard),
    LabelingRoad((StableRoadID, Direction), Wizard),
    LabelingIntersection(StableIntersectionID, Wizard),
    CreatingRoad(StableIntersectionID),
    EditingLanes(StableRoadID, Wizard),
    EditingRoadAttribs(StableRoadID, Wizard),
    SavingModel(Wizard),
    // bool is if key is down
    SelectingRectangle(Pt2D, Pt2D, bool),
}

impl UI {
    fn new(ctx: &EventCtx) -> UI {
        let mut args = CmdArgs::new();
        let load = args.optional_free();
        let exclude_bldgs = args.enabled("--nobldgs");
        let edit_fixes = args.optional("--fixes");
        args.done();

        let model = if let Some(path) = load {
            Model::import(&path, exclude_bldgs, edit_fixes, ctx.prerender)
        } else {
            Model::blank()
        };
        UI {
            model,
            state: State::Viewing,
            osd: Text::new(),
        }
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            self.model.handle_mouseover(ctx);
        }

        match self.state {
            State::MovingIntersection(id) => {
                if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                    self.model.move_i(id, cursor, ctx.prerender);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.state = State::Viewing;
                    }
                }
            }
            State::MovingBuilding(id) => {
                if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                    self.model.move_b(id, cursor, ctx.prerender);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.state = State::Viewing;
                    }
                }
            }
            State::MovingRoadPoint(r, idx) => {
                if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                    self.model.move_r_pt(r, idx, cursor, ctx.prerender);
                    if ctx.input.key_released(Key::LeftControl) {
                        self.state = State::Viewing;
                    }
                }
            }
            State::LabelingBuilding(id, ref mut wizard) => {
                if let Some(label) = wizard.wrap(ctx).input_string_prefilled(
                    "Label the building",
                    self.model.get_b_label(id).unwrap_or_else(String::new),
                ) {
                    self.model.set_b_label(id, label, ctx.prerender);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::LabelingRoad(pair, ref mut wizard) => {
                if let Some(label) = wizard.wrap(ctx).input_string_prefilled(
                    "Label this side of the road",
                    self.model.get_r_label(pair).unwrap_or_else(String::new),
                ) {
                    self.model.set_r_label(pair, label, ctx.prerender);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::LabelingIntersection(id, ref mut wizard) => {
                if let Some(label) = wizard.wrap(ctx).input_string_prefilled(
                    "Label the intersection",
                    self.model.get_i_label(id).unwrap_or_else(String::new),
                ) {
                    self.model.set_i_label(id, label, ctx.prerender);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::CreatingRoad(i1) => {
                if ctx.input.key_pressed(Key::Escape, "stop defining road") {
                    self.state = State::Viewing;
                    self.model.handle_mouseover(ctx);
                } else if let Some(ID::Intersection(i2)) = self.model.get_selection() {
                    if i1 != i2 && ctx.input.key_pressed(Key::R, "finalize road") {
                        self.model.create_r(i1, i2, ctx.prerender);
                        self.state = State::Viewing;
                        self.model.handle_mouseover(ctx);
                    }
                }
            }
            State::EditingLanes(id, ref mut wizard) => {
                if let Some(s) = wizard
                    .wrap(ctx)
                    .input_string_prefilled("Specify the lanes", self.model.get_road_spec(id))
                {
                    self.model.edit_lanes(id, s, ctx.prerender);
                    self.state = State::Viewing;
                    self.model.handle_mouseover(ctx);
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                    self.model.handle_mouseover(ctx);
                }
            }
            State::EditingRoadAttribs(id, ref mut wizard) => {
                let (orig_name, orig_speed) = self.model.get_r_name_and_speed(id);

                let mut wiz = wizard.wrap(ctx);
                let mut done = false;
                if let Some(n) = wiz.input_string_prefilled("Name the road", orig_name) {
                    if let Some(s) = wiz.input_string_prefilled("What speed limit?", orig_speed) {
                        self.model.set_r_name_and_speed(id, n, s, ctx.prerender);
                        done = true;
                    }
                }
                if done || wizard.aborted() {
                    self.state = State::Viewing;
                    self.model.handle_mouseover(ctx);
                }
            }
            State::SavingModel(ref mut wizard) => {
                if let Some(name) = wizard.wrap(ctx).input_string("Name the synthetic map") {
                    self.model.map.name = name;
                    self.model.export();
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::Viewing => {
                let cursor = ctx.canvas.get_cursor_in_map_space();
                if let Some(ID::Intersection(i)) = self.model.get_selection() {
                    if ctx.input.key_pressed(Key::LeftControl, "move intersection") {
                        self.state = State::MovingIntersection(i);
                    } else if ctx.input.key_pressed(Key::R, "create road") {
                        self.state = State::CreatingRoad(i);
                    } else if ctx.input.key_pressed(Key::Backspace, "delete intersection") {
                        self.model.delete_i(i);
                        self.model.handle_mouseover(ctx);
                    } else if ctx.input.key_pressed(Key::T, "toggle intersection type") {
                        self.model.toggle_i_type(i, ctx.prerender);
                    } else if ctx.input.key_pressed(Key::L, "label intersection") {
                        self.state = State::LabelingIntersection(i, Wizard::new());
                    }
                } else if let Some(ID::Building(b)) = self.model.get_selection() {
                    if ctx.input.key_pressed(Key::LeftControl, "move building") {
                        self.state = State::MovingBuilding(b);
                    } else if ctx.input.key_pressed(Key::Backspace, "delete building") {
                        self.model.delete_b(b);
                        self.model.handle_mouseover(ctx);
                    } else if ctx.input.key_pressed(Key::L, "label building") {
                        self.state = State::LabelingBuilding(b, Wizard::new());
                    }
                } else if let Some(ID::Lane(r, dir, _)) = self.model.get_selection() {
                    if ctx
                        .input
                        .key_pressed(Key::Backspace, &format!("delete road {}", r))
                    {
                        self.model.delete_r(r);
                        self.model.handle_mouseover(ctx);
                    } else if ctx.input.key_pressed(Key::E, "edit lanes") {
                        self.state = State::EditingLanes(r, Wizard::new());
                    } else if ctx.input.key_pressed(Key::N, "edit name/speed") {
                        self.state = State::EditingRoadAttribs(r, Wizard::new());
                    } else if ctx.input.key_pressed(Key::S, "swap lanes") {
                        self.model.swap_lanes(r, ctx.prerender);
                        self.model.handle_mouseover(ctx);
                    } else if ctx.input.key_pressed(Key::L, "label side of the road") {
                        self.state = State::LabelingRoad((r, dir), Wizard::new());
                    } else if self.model.showing_pts.is_none()
                        && ctx.input.key_pressed(Key::P, "move road points")
                    {
                        self.model.show_r_points(r, ctx.prerender);
                        self.model.handle_mouseover(ctx);
                    } else if ctx.input.key_pressed(Key::M, "merge road") {
                        self.model.merge_r(r, ctx.prerender);
                        self.model.handle_mouseover(ctx);
                    }
                } else if let Some(ID::RoadPoint(r, idx)) = self.model.get_selection() {
                    if ctx.input.key_pressed(Key::LeftControl, "move point") {
                        self.state = State::MovingRoadPoint(r, idx);
                    } else if ctx.input.key_pressed(Key::Backspace, "delete point") {
                        self.model.delete_r_pt(r, idx, ctx.prerender);
                        self.model.handle_mouseover(ctx);
                    }
                } else if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
                    process::exit(0);
                } else if ctx.input.key_pressed(Key::S, "save") {
                    if self.model.map.name != "" {
                        self.model.export();
                    } else {
                        self.state = State::SavingModel(Wizard::new());
                    }
                } else if ctx.input.key_pressed(Key::F, "save map fixes") {
                    self.model.save_fixes();
                } else if cursor.is_some() && ctx.input.key_pressed(Key::I, "create intersection") {
                    self.model.create_i(cursor.unwrap(), ctx.prerender);
                    self.model.handle_mouseover(ctx);
                // TODO Silly bug: Mouseover doesn't actually work! I think the cursor being
                // dead-center messes up the precomputed triangles.
                } else if cursor.is_some() && ctx.input.key_pressed(Key::B, "create building") {
                    self.model.create_b(cursor.unwrap(), ctx.prerender);
                    self.model.handle_mouseover(ctx);
                } else if cursor.is_some() && ctx.input.key_pressed(Key::LeftShift, "select area") {
                    self.state = State::SelectingRectangle(cursor.unwrap(), cursor.unwrap(), true);
                } else if self.model.showing_pts.is_some()
                    && ctx.input.key_pressed(Key::P, "stop moving road points")
                {
                    self.model.stop_showing_pts();
                }
            }
            State::SelectingRectangle(pt1, ref mut pt2, ref mut keydown) => {
                if ctx.input.key_pressed(Key::LeftShift, "select area") {
                    *keydown = true;
                } else if ctx.input.key_released(Key::LeftShift) {
                    *keydown = false;
                }

                if *keydown {
                    if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                        *pt2 = cursor;
                    }
                }
                if ctx.input.key_pressed(Key::Escape, "stop selecting area") {
                    self.state = State::Viewing;
                } else if ctx
                    .input
                    .key_pressed(Key::Backspace, "delete everything area")
                {
                    if let Some(rect) = Polygon::rectangle_two_corners(pt1, *pt2) {
                        self.model.delete_everything_inside(rect);
                        self.model.handle_mouseover(ctx);
                    }
                    self.state = State::Viewing;
                }
            }
        }

        self.osd = Text::new();
        ctx.input.populate_osd(&mut self.osd);
        EventLoopMode::InputOnly
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.model.draw(g);

        match self.state {
            State::CreatingRoad(i1) => {
                if let Some(cursor) = g.get_cursor_in_map_space() {
                    if let Some(l) = Line::maybe_new(self.model.get_i_center(i1), cursor) {
                        g.draw_line(Color::GREEN, Distance::meters(5.0), &l);
                    }
                }
            }
            State::LabelingBuilding(_, ref wizard)
            | State::LabelingRoad(_, ref wizard)
            | State::LabelingIntersection(_, ref wizard)
            | State::EditingLanes(_, ref wizard)
            | State::EditingRoadAttribs(_, ref wizard)
            | State::SavingModel(ref wizard) => {
                wizard.draw(g);
            }
            State::Viewing => {
                if let Some(ID::Lane(id, _, _)) = self.model.get_selection() {
                    let mut txt = Text::new();
                    for (k, v) in self.model.get_tags(id) {
                        txt.add_appended(vec![
                            Line(k).fg(Color::RED),
                            Line(" = "),
                            Line(v).fg(Color::CYAN),
                        ]);
                    }
                    g.draw_blocking_text(
                        &txt,
                        (
                            ezgui::HorizontalAlignment::Right,
                            ezgui::VerticalAlignment::Top,
                        ),
                    );
                }
            }
            State::MovingIntersection(_)
            | State::MovingBuilding(_)
            | State::MovingRoadPoint(_, _) => {}
            State::SelectingRectangle(pt1, pt2, _) => {
                if let Some(rect) = Polygon::rectangle_two_corners(pt1, pt2) {
                    g.draw_polygon(Color::BLUE.alpha(0.5), &rect);
                }
            }
        };

        g.draw_blocking_text(&self.osd, ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    ezgui::run("Synthetic map editor", 1024.0, 768.0, |ctx| UI::new(ctx));
}

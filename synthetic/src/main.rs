mod model;

use crate::model::{BuildingID, Direction, Model, ID};
use aabb_quadtree::QuadTree;
use ezgui::{Canvas, Color, EventLoopMode, GfxCtx, Key, Text, UserInput, Wizard, GUI};
use geom::Line;
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use std::{env, process};

struct UI {
    canvas: Canvas,
    model: Model,
    quadtree: Option<QuadTree<ID>>,
    state: State,
}

enum State {
    Viewing,
    MovingIntersection(StableIntersectionID),
    MovingBuilding(BuildingID),
    LabelingBuilding(BuildingID, Wizard),
    LabelingRoad((StableRoadID, Direction), Wizard),
    LabelingIntersection(StableIntersectionID, Wizard),
    CreatingRoad(StableIntersectionID),
    EditingRoad(StableRoadID, Wizard),
    SavingModel(Wizard),
}

impl UI {
    fn new(load: Option<&String>) -> UI {
        let (model, quadtree): (Model, Option<QuadTree<ID>>) = if let Some(path) = load {
            if path.contains("raw_maps/") {
                let (m, q) = Model::import(path);
                (m, Some(q))
            } else {
                (
                    abstutil::read_json(path).expect(&format!("Couldn't load {}", path)),
                    None,
                )
            }
        } else {
            (Model::new(), None)
        };
        UI {
            canvas: Canvas::new(1024, 768),
            model,
            quadtree,
            state: State::Viewing,
        }
    }
}

impl GUI<Text> for UI {
    fn event(&mut self, input: &mut UserInput) -> (EventLoopMode, Text) {
        self.canvas.handle_event(input);
        let cursor = {
            if let Some(c) = self.canvas.get_cursor_in_map_space() {
                c
            } else {
                return (EventLoopMode::InputOnly, Text::new());
            }
        };
        let selected = self
            .model
            .mouseover_something(&self.canvas, self.quadtree.as_ref());

        match self.state {
            State::MovingIntersection(id) => {
                self.model.move_i(id, cursor);
                if input.key_released(Key::LeftControl) {
                    self.state = State::Viewing;
                }
            }
            State::MovingBuilding(id) => {
                self.model.move_b(id, cursor);
                if input.key_released(Key::LeftControl) {
                    self.state = State::Viewing;
                }
            }
            State::LabelingBuilding(id, ref mut wizard) => {
                if let Some(label) = wizard.wrap(input, &self.canvas).input_string_prefilled(
                    "Label the building",
                    self.model.get_b_label(id).unwrap_or_else(String::new),
                ) {
                    self.model.set_b_label(id, label);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::LabelingRoad(pair, ref mut wizard) => {
                if let Some(label) = wizard.wrap(input, &self.canvas).input_string_prefilled(
                    "Label this side of the road",
                    self.model.get_r_label(pair).unwrap_or_else(String::new),
                ) {
                    self.model.set_r_label(pair, label);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::LabelingIntersection(id, ref mut wizard) => {
                if let Some(label) = wizard.wrap(input, &self.canvas).input_string_prefilled(
                    "Label the intersection",
                    self.model.get_i_label(id).unwrap_or_else(String::new),
                ) {
                    self.model.set_i_label(id, label);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::CreatingRoad(i1) => {
                if input.key_pressed(Key::Escape, "stop defining road") {
                    self.state = State::Viewing;
                } else if let Some(ID::Intersection(i2)) = selected {
                    if i1 != i2 && input.key_pressed(Key::R, "finalize road") {
                        self.model.create_road(i1, i2);
                        self.state = State::Viewing;
                    }
                }
            }
            State::EditingRoad(id, ref mut wizard) => {
                if let Some(s) = wizard
                    .wrap(input, &self.canvas)
                    .input_string_prefilled("Specify the lanes", self.model.get_lanes(id))
                {
                    self.model.edit_lanes(id, s);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::SavingModel(ref mut wizard) => {
                if let Some(name) = wizard
                    .wrap(input, &self.canvas)
                    .input_string("Name the synthetic map")
                {
                    self.model.name = Some(name);
                    self.model.save();
                    self.model.export();
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::Viewing => {
                if let Some(ID::Intersection(i)) = selected {
                    if input.key_pressed(Key::LeftControl, "move intersection") {
                        self.state = State::MovingIntersection(i);
                    } else if input.key_pressed(Key::R, "create road") {
                        self.state = State::CreatingRoad(i);
                    } else if input.key_pressed(Key::Backspace, "delete intersection") {
                        self.model.remove_i(i);
                    } else if input.key_pressed(Key::T, "toggle intersection type") {
                        self.model.toggle_i_type(i);
                    } else if input.key_pressed(Key::L, "label intersection") {
                        self.state = State::LabelingIntersection(i, Wizard::new());
                    }
                } else if let Some(ID::Building(b)) = selected {
                    if input.key_pressed(Key::LeftControl, "move building") {
                        self.state = State::MovingBuilding(b);
                    } else if input.key_pressed(Key::Backspace, "delete building") {
                        self.model.remove_b(b);
                    } else if input.key_pressed(Key::L, "label building") {
                        self.state = State::LabelingBuilding(b, Wizard::new());
                    }
                } else if let Some(ID::Road(r)) = selected {
                    let (_, dir) = self.model.mouseover_road(r, cursor).unwrap();
                    if input.key_pressed(Key::Backspace, &format!("delete road {}", r)) {
                        self.model.remove_road(r);
                    } else if input.key_pressed(Key::E, "edit lanes") {
                        self.state = State::EditingRoad(r, Wizard::new());
                    } else if input.key_pressed(Key::S, "swap lanes") {
                        self.model.swap_lanes(r);
                    } else if input.key_pressed(Key::L, "label side of the road") {
                        self.state = State::LabelingRoad((r, dir), Wizard::new());
                    }
                } else if input.unimportant_key_pressed(Key::Escape, "quit") {
                    process::exit(0);
                } else if input.key_pressed(Key::S, "save") {
                    if self.model.name.is_some() {
                        self.model.save();
                        self.model.export();
                    } else {
                        self.state = State::SavingModel(Wizard::new());
                    }
                } else if input.key_pressed(Key::I, "create intersection") {
                    self.model.create_i(cursor);
                } else if input.key_pressed(Key::B, "create building") {
                    self.model.create_b(cursor);
                }
            }
        }

        let mut osd = Text::new();
        input.populate_osd(&mut osd);
        (EventLoopMode::InputOnly, osd)
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, osd: &Text) {
        self.model.draw(g, &self.canvas, self.quadtree.as_ref());

        match self.state {
            State::CreatingRoad(i1) => {
                if let Some(cursor) = self.canvas.get_cursor_in_map_space() {
                    g.draw_line(
                        Color::GREEN,
                        5.0,
                        &Line::new(self.model.get_i_center(i1), cursor),
                    );
                }
            }
            State::LabelingBuilding(_, ref wizard)
            | State::LabelingRoad(_, ref wizard)
            | State::LabelingIntersection(_, ref wizard)
            | State::EditingRoad(_, ref wizard)
            | State::SavingModel(ref wizard) => {
                wizard.draw(g, &self.canvas);
            }
            _ => {}
        };

        self.canvas
            .draw_blocking_text(g, osd.clone(), ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run(UI::new(args.get(1)), "Synthetic map editor");
}

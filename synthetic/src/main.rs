extern crate abstutil;
extern crate dimensioned;
extern crate ezgui;
extern crate geom;
extern crate map_model;
extern crate piston;
#[macro_use]
extern crate serde_derive;

mod model;

use ezgui::{Canvas, Color, GfxCtx, Text, UserInput, Wizard, GUI};
use geom::Line;
use model::{BuildingID, IntersectionID, Model, RoadID};
use piston::input::Key;
use std::{env, process};

const KEY_CATEGORY: &str = "";

struct UI {
    canvas: Canvas,
    model: Model,
    state: State,
}

enum State {
    Viewing,
    MovingIntersection(IntersectionID),
    MovingBuilding(BuildingID),
    CreatingRoad(IntersectionID),
    EditingRoad(RoadID, Wizard),
    SavingModel(Wizard),
}

impl UI {
    fn new(load: Option<&String>) -> UI {
        let model: Model = if let Some(path) = load {
            abstutil::read_json(path).expect(&format!("Couldn't load {}", path))
        } else {
            Model::new()
        };
        UI {
            canvas: Canvas::new(),
            model,
            state: State::Viewing,
        }
    }
}

impl GUI for UI {
    fn event(&mut self, mut input: UserInput, osd: &mut Text) {
        self.canvas.handle_event(&mut input);
        let cursor = self.canvas.get_cursor_in_map_space();

        // Most of the time, we can directly overwrite the state below. But when we can't clone the
        // state enum (like for Wizards), we have to use this.
        let mut new_state: Option<State> = None;

        match self.state {
            State::MovingIntersection(id) => {
                self.model.move_i(id, cursor);
                if input.key_released(Key::LCtrl) {
                    self.state = State::Viewing;
                }
            }
            State::MovingBuilding(id) => {
                self.model.move_b(id, cursor);
                if input.key_released(Key::LCtrl) {
                    self.state = State::Viewing;
                }
            }
            State::CreatingRoad(i1) => {
                if input.key_pressed(Key::Escape, "stop defining road") {
                    self.state = State::Viewing;
                } else if let Some(i2) = self.model.mouseover_intersection(cursor) {
                    if i1 != i2 {
                        if input.key_pressed(Key::R, "finalize road") {
                            self.model.create_road(i1, i2);
                            self.state = State::Viewing;
                        }
                    }
                }
            }
            State::EditingRoad(id, ref mut wizard) => {
                if let Some(s) = wizard
                    .wrap(&mut input)
                    .input_string_prefilled("Specify the lanes", self.model.get_lanes(id))
                {
                    self.model.edit_lanes(id, s);
                    new_state = Some(State::Viewing);
                } else if wizard.aborted() {
                    new_state = Some(State::Viewing);
                }
            }
            State::SavingModel(ref mut wizard) => {
                if let Some(name) = wizard
                    .wrap(&mut input)
                    .input_string("Name the synthetic map")
                {
                    self.model.name = Some(name);
                    self.model.save();
                    new_state = Some(State::Viewing);
                } else if wizard.aborted() {
                    new_state = Some(State::Viewing);
                }
            }
            State::Viewing => {
                if input.unimportant_key_pressed(Key::Escape, KEY_CATEGORY, "quit") {
                    process::exit(0);
                }

                if input.key_pressed(Key::S, "save") {
                    if self.model.name.is_some() {
                        self.model.save();
                    } else {
                        self.state = State::SavingModel(Wizard::new());
                    }
                } else if self.model.name.is_some() && input.key_pressed(Key::X, "export map") {
                    self.model.export();
                } else if input.key_pressed(Key::I, "create intersection") {
                    self.model.create_i(cursor);
                } else if input.key_pressed(Key::B, "create building") {
                    self.model.create_b(cursor);
                } else if let Some(i) = self.model.mouseover_intersection(cursor) {
                    if input.key_pressed(Key::LCtrl, "move intersection") {
                        self.state = State::MovingIntersection(i);
                    } else if input.key_pressed(Key::R, "create road") {
                        self.state = State::CreatingRoad(i);
                    } else if input.key_pressed(Key::Backspace, "delete intersection") {
                        self.model.remove_i(i);
                    } else if input.key_pressed(Key::T, "toggle intersection type") {
                        self.model.toggle_i_type(i);
                    }
                } else if let Some(b) = self.model.mouseover_building(cursor) {
                    if input.key_pressed(Key::LCtrl, "move building") {
                        self.state = State::MovingBuilding(b);
                    } else if input.key_pressed(Key::Backspace, "delete building") {
                        self.model.remove_b(b);
                    }
                } else if let Some(r) = self.model.mouseover_road(cursor) {
                    if input.key_pressed(Key::Backspace, "delete road") {
                        self.model.remove_road(r);
                    } else if input.key_pressed(Key::E, "edit lanes") {
                        self.state = State::EditingRoad(r, Wizard::new());
                    }
                }
            }
        }

        if let Some(s) = new_state {
            self.state = s;
        }

        input.populate_osd(osd);
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, osd: Text) {
        self.model.draw(g);

        match self.state {
            State::CreatingRoad(i1) => {
                g.draw_line(
                    Color::GREEN,
                    5.0,
                    &Line::new(
                        self.model.get_i_center(i1),
                        self.canvas.get_cursor_in_map_space(),
                    ),
                );
            }
            State::EditingRoad(_, ref wizard) | State::SavingModel(ref wizard) => {
                wizard.draw(g, &self.canvas);
            }
            _ => {}
        };

        self.canvas.draw_text(g, osd, ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run(UI::new(args.get(1)), "Synthetic map editor", 1024, 768);
}

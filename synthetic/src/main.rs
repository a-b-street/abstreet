extern crate abstutil;
extern crate ezgui;
extern crate geom;
extern crate piston;

mod model;

use ezgui::{Canvas, Color, GfxCtx, Text, UserInput, GUI};
use geom::Line;
use model::{BuildingID, IntersectionID, Model};
use piston::input::Key;
use std::process;

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
}

impl UI {
    // TODO load stuff
    fn new() -> UI {
        UI {
            canvas: Canvas::new(),
            model: Model::new(),
            state: State::Viewing,
        }
    }
}

impl GUI for UI {
    fn event(&mut self, mut input: UserInput, osd: &mut Text) {
        self.canvas.handle_event(&mut input);
        let cursor = self.canvas.get_cursor_in_map_space();

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
            State::Viewing => {
                if input.unimportant_key_pressed(Key::Escape, KEY_CATEGORY, "quit") {
                    process::exit(0);
                }

                if input.key_pressed(Key::I, "create intersection") {
                    self.model.create_i(cursor);
                }
                if input.key_pressed(Key::B, "create building") {
                    self.model.create_b(cursor);
                }

                if let Some(i) = self.model.mouseover_intersection(cursor) {
                    if input.key_pressed(Key::LCtrl, "move intersection") {
                        self.state = State::MovingIntersection(i);
                    }

                    if input.key_pressed(Key::R, "create road") {
                        self.state = State::CreatingRoad(i);
                    }

                    if input.key_pressed(Key::Backspace, "delete intersection") {
                        self.model.remove_i(i);
                    }
                } else if let Some(b) = self.model.mouseover_building(cursor) {
                    if input.key_pressed(Key::LCtrl, "move building") {
                        self.state = State::MovingBuilding(b);
                    }

                    if input.key_pressed(Key::Backspace, "delete building") {
                        self.model.remove_b(b);
                    }
                } else if let Some(r) = self.model.mouseover_road(cursor) {
                    if input.key_pressed(Key::Backspace, "delete road") {
                        self.model.remove_road(r);
                    }
                }
            }
        }

        input.populate_osd(osd);
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, osd: Text) {
        self.model.draw(g);

        if let State::CreatingRoad(i1) = self.state {
            g.draw_line(
                Color::GREEN,
                model::ROAD_WIDTH,
                &Line::new(
                    self.model.get_i_center(i1),
                    self.canvas.get_cursor_in_map_space(),
                ),
            );
        }

        self.canvas.draw_text(g, osd, ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    ezgui::run(UI::new(), "Synthetic map editor", 1024, 768);
}

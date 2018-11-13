extern crate abstutil;
extern crate ezgui;
extern crate geom;
extern crate piston;

mod model;

use ezgui::{Canvas, Color, GfxCtx, Text, UserInput, GUI};
use geom::Line;
use model::{Intersection, IntersectionID, Model};
use piston::input::Key;
use std::process;

const KEY_CATEGORY: &str = "";

struct UI {
    canvas: Canvas,
    model: Model,

    moving_intersection: Option<IntersectionID>,
    creating_road: Option<IntersectionID>,
}

impl UI {
    // TODO load stuff
    fn new() -> UI {
        UI {
            canvas: Canvas::new(),
            model: Model::new(),

            moving_intersection: None,
            creating_road: None,
        }
    }
}

impl GUI for UI {
    fn event(&mut self, mut input: UserInput, _osd: &mut Text) {
        self.canvas.handle_event(&mut input);
        let cursor = self.canvas.get_cursor_in_map_space();

        if let Some(id) = self.moving_intersection {
            if input.key_released(Key::LCtrl) {
                self.moving_intersection = None;
            }
            self.model.intersections.get_mut(&id).unwrap().center = cursor;
        } else if let Some(i1) = self.creating_road {
            if input.unimportant_key_pressed(Key::Escape, KEY_CATEGORY, "stop defining road") {
                self.creating_road = None;
            } else if input.unimportant_key_pressed(Key::R, KEY_CATEGORY, "finalize road") {
                if let Some(i2) = self.model.mouseover_intersection(cursor) {
                    if i1 != i2 {
                        self.model.create_road(i1, i2);
                        self.creating_road = None;
                    }
                }
            }
        } else {
            if input.unimportant_key_pressed(Key::Escape, KEY_CATEGORY, "quit") {
                process::exit(0);
            }

            if input.unimportant_key_pressed(Key::I, KEY_CATEGORY, "create intersection") {
                let id = self.model.intersections.len();
                self.model
                    .intersections
                    .insert(id, Intersection { center: cursor });
            }

            if input.unimportant_key_pressed(Key::LCtrl, KEY_CATEGORY, "move intersection") {
                self.moving_intersection = self.model.mouseover_intersection(cursor);
            }
            if input.unimportant_key_pressed(Key::R, KEY_CATEGORY, "create road") {
                self.creating_road = self.model.mouseover_intersection(cursor);
            }
            if input.unimportant_key_pressed(Key::Backspace, KEY_CATEGORY, "delete something") {
                if let Some(i) = self.model.mouseover_intersection(cursor) {
                    // TODO No references
                    self.model.intersections.remove(&i);
                }
            }
        }
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, _osd: Text) {
        self.model.draw(g);

        if let Some(i1) = self.creating_road {
            g.draw_line(
                Color::GREEN,
                5.0,
                &Line::new(
                    self.model.intersections[&i1].center,
                    self.canvas.get_cursor_in_map_space(),
                ),
            );
        }
    }
}

fn main() {
    ezgui::run(UI::new(), "Synthetic map editor", 1024, 768);
}

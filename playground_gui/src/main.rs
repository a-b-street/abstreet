mod common;
mod debug_intersection;
mod debug_polygon_drawing;
mod debug_polyline;
mod moving_polyline;
mod trim_polyline;

use ezgui::{Canvas, EventLoopMode, GfxCtx, Key, Text, UserInput, GUI};
use geom::Pt2D;
use std::f64;
use std::process;

pub struct UI {
    canvas: Canvas,
    p3_offset: (f64, f64),
    show_labels: bool,
    current_mode: usize,
}

impl UI {
    pub fn new() -> UI {
        let mut canvas = Canvas::new();
        // Start with mode 1's settings
        canvas.window_size.width = 1024;
        canvas.window_size.height = 768;
        canvas.cam_zoom = 1.0;
        canvas.center_on_map_pt(Pt2D::new(305.0, 324.0));

        UI {
            canvas,
            p3_offset: (200.0, 150.0),
            show_labels: true,
            current_mode: 1,
        }
    }
}

impl GUI<()> for UI {
    fn event(&mut self, input: &mut UserInput) -> (EventLoopMode, ()) {
        if input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }
        let speed = 5.0;
        if input.unimportant_key_pressed(Key::H, "left") {
            self.p3_offset.0 -= speed;
        }
        if input.unimportant_key_pressed(Key::J, "down") {
            self.p3_offset.1 += speed;
        }
        if input.unimportant_key_pressed(Key::K, "up") {
            self.p3_offset.1 -= speed;
        }
        if input.unimportant_key_pressed(Key::L, "right") {
            self.p3_offset.0 += speed;
        }
        if input.unimportant_key_pressed(Key::P, "toggle labels") {
            self.show_labels = !self.show_labels;
        }
        if input.unimportant_key_pressed(Key::C, "print current camera state") {
            println!("cam_zoom = {}", self.canvas.cam_zoom);
            println!("center_on_map_pt({})", self.canvas.center_to_map_pt());
        }
        if input.unimportant_key_pressed(Key::Num1, "switch to mode 1") {
            self.current_mode = 1;
            self.canvas.cam_zoom = 1.0;
            self.canvas.center_on_map_pt(Pt2D::new(305.0, 324.0));
        }
        if input.unimportant_key_pressed(Key::Num2, "switch to mode 2") {
            self.current_mode = 2;
            self.canvas.cam_zoom = 10.0;
            self.canvas.center_on_map_pt(Pt2D::new(1352.0, 403.0));
        }
        if input.unimportant_key_pressed(Key::Num3, "switch to mode 3") {
            self.current_mode = 3;
            self.canvas.cam_zoom = 3.8;
            self.canvas.center_on_map_pt(Pt2D::new(2025.0, 1277.0));
        }
        if input.unimportant_key_pressed(Key::Num4, "switch to mode 4") {
            self.current_mode = 4;
            self.canvas.cam_zoom = 10.5;
            self.canvas.center_on_map_pt(Pt2D::new(122.0, 166.0));
        }
        if input.unimportant_key_pressed(Key::Num5, "switch to mode 5") {
            self.current_mode = 5;
            self.canvas.cam_zoom = 19.0;
            self.canvas.center_on_map_pt(Pt2D::new(1166.0, 766.0));
        }

        self.canvas.handle_event(input);

        (EventLoopMode::InputOnly, ())
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, _: &()) {
        g.clear(common::WHITE);

        let mut labels: Vec<(Pt2D, String)> = Vec::new();

        match self.current_mode {
            1 => moving_polyline::run(self.p3_offset, g, &mut labels),
            2 => trim_polyline::run(g),
            3 => debug_intersection::run(g),
            4 => debug_polyline::run(g, &mut labels),
            5 => debug_polygon_drawing::run(g, &mut labels),
            x => panic!("Impossible current_mode {}", x),
        };

        // TODO detect "breakages" by dist from p2 to p2_c beyond threshold
        // TODO still try the angle bisection method

        if self.show_labels {
            for (pt, label) in labels.into_iter() {
                self.canvas.draw_text_at(g, Text::from_line(label), pt);
            }
        }
    }
}

fn main() {
    ezgui::run(UI::new(), "GUI Playground", 1024, 768);
}

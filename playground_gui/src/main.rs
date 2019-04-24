mod common;
mod debug_intersection;
mod debug_polygon_drawing;
mod debug_polyline;
mod moving_polyline;
mod trim_polyline;

use ezgui::{Canvas, EventCtx, EventLoopMode, GfxCtx, Key, Text, GUI};
use geom::Pt2D;
use std::process;

pub struct UI {
    p3_offset: (f64, f64),
    show_labels: bool,
    current_mode: usize,
}

impl UI {
    pub fn new(canvas: &mut Canvas) -> UI {
        canvas.center_on_map_pt(Pt2D::new(305.0, 324.0));

        UI {
            p3_offset: (200.0, 150.0),
            show_labels: true,
            current_mode: 1,
        }
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
            process::exit(0);
        }
        let speed = 5.0;
        if ctx.input.unimportant_key_pressed(Key::H, "left") {
            self.p3_offset.0 -= speed;
        }
        if ctx.input.unimportant_key_pressed(Key::J, "down") {
            self.p3_offset.1 += speed;
        }
        if ctx.input.unimportant_key_pressed(Key::K, "up") {
            self.p3_offset.1 -= speed;
        }
        if ctx.input.unimportant_key_pressed(Key::L, "right") {
            self.p3_offset.0 += speed;
        }
        if ctx.input.unimportant_key_pressed(Key::P, "toggle labels") {
            self.show_labels = !self.show_labels;
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::C, "print current camera state")
        {
            println!("cam_zoom = {}", ctx.canvas.cam_zoom);
            println!("center_on_map_pt({})", ctx.canvas.center_to_map_pt());
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::Num1, "switch to mode 1")
        {
            self.current_mode = 1;
            ctx.canvas.cam_zoom = 1.0;
            ctx.canvas.center_on_map_pt(Pt2D::new(305.0, 324.0));
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::Num2, "switch to mode 2")
        {
            self.current_mode = 2;
            ctx.canvas.cam_zoom = 10.0;
            ctx.canvas.center_on_map_pt(Pt2D::new(1352.0, 403.0));
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::Num3, "switch to mode 3")
        {
            self.current_mode = 3;
            ctx.canvas.cam_zoom = 3.8;
            ctx.canvas.center_on_map_pt(Pt2D::new(2025.0, 1277.0));
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::Num4, "switch to mode 4")
        {
            self.current_mode = 4;
            ctx.canvas.cam_zoom = 10.5;
            ctx.canvas.center_on_map_pt(Pt2D::new(122.0, 166.0));
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::Num5, "switch to mode 5")
        {
            self.current_mode = 5;
            ctx.canvas.cam_zoom = 19.0;
            ctx.canvas.center_on_map_pt(Pt2D::new(1166.0, 766.0));
        }

        ctx.canvas.handle_event(ctx.input);

        EventLoopMode::InputOnly
    }

    fn draw(&self, g: &mut GfxCtx) {
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
                g.draw_text_at(&Text::from_line(label), pt);
            }
        }
    }
}

fn main() {
    ezgui::run("GUI Playground", 1024.0, 768.0, |canvas, _| UI::new(canvas));
}

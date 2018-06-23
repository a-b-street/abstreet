use animation;
use ezgui::GfxCtx;
use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use graphics;
use graphics::types::Color;
use gui;
use piston::input::Key;
use piston::window::Size;
use std::process;

const WHITE: Color = [1.0; 4];
const RED: Color = [1.0, 0.0, 0.0, 0.8];

pub struct UI {
    canvas: Canvas,
}

impl UI {
    pub fn new() -> UI {
        UI {
            canvas: Canvas::new(),
        }
    }
}

impl gui::GUI for UI {
    fn event(
        mut self,
        input: &mut UserInput,
        _window_size: &Size,
    ) -> (UI, animation::EventLoopMode) {
        if input.unimportant_key_pressed(Key::Escape, "Press escape to quit") {
            process::exit(0);
        }

        self.canvas.handle_event(input.use_event_directly());

        (self, animation::EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, _input: UserInput) {
        graphics::clear(WHITE, g.gfx);
        g.ctx = self.canvas.get_transformed_context(&g.orig_ctx);

        let pt1 = (100.0, 100.0);
        let pt2 = (110.0, 200.0);
        let pt3 = (300.0, 250.0);

        line(g, pt1, pt2);
        line(g, pt2, pt3);

        self.label(g, pt1, "pt1");
        self.label(g, pt2, "pt2");
        self.label(g, pt3, "pt3");
    }
}

impl UI {
    fn label(&self, g: &mut GfxCtx, pt: (f64, f64), text: &str) {
        self.canvas
            .draw_text_at(g, &vec![text.to_string()], pt.0, pt.1);
    }
}

fn line(g: &mut GfxCtx, pt1: (f64, f64), pt2: (f64, f64)) {
    graphics::Line::new_round(RED, 1.0).draw(
        [pt1.0, pt1.1, pt2.0, pt2.1],
        &g.ctx.draw_state,
        g.ctx.transform,
        g.gfx,
    );
}

use animation;
use ezgui::GfxCtx;
use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use graphics;
use graphics::types::Color;
use gui;
use piston::input::Key;
use piston::window::Size;
use std::f64;
use std::process;

const WHITE: Color = [1.0; 4];
const RED: Color = [1.0, 0.0, 0.0, 0.8];
const GREEN: Color = [0.0, 1.0, 0.0, 0.8];
const BLUE: Color = [0.0, 0.0, 1.0, 0.8];

pub struct UI {
    canvas: Canvas,
    p3_offset: (f64, f64),
}

impl UI {
    pub fn new() -> UI {
        UI {
            canvas: Canvas::new(),
            p3_offset: (200.0, 150.0),
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

        self.canvas.handle_event(input.use_event_directly());

        (self, animation::EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, _input: UserInput) {
        graphics::clear(WHITE, g.gfx);
        g.ctx = self.canvas.get_transformed_context(&g.orig_ctx);

        let thin = 1.0;
        let thick = 5.0;
        let shift_away = 50.0;

        // TODO detect "breakages" by dist from p2 to p2_c beyond threshold
        // TODO automatic labels for all points, but be able to toggle them
        // TODO bezier curves could be ideal for both drawing and car paths, but no easy way to
        // try them out in piston
        // TODO figure out polygons too

        let p1 = (100.0, 100.0);
        let p2 = (110.0, 200.0);
        let p3 = (p1.0 + self.p3_offset.0, p1.1 + self.p3_offset.1);

        line(g, p1, p2, thick, RED);
        line(g, p2, p3, thick, RED);

        // Two lanes on one side of the road
        let (p1_a, p2_a) = shift_line(shift_away, p1, p2);
        let (p2_b, p3_b) = shift_line(shift_away, p2, p3);
        let p2_c = line_intersection((p1_a, p2_a), (p2_b, p3_b));

        line(g, p1_a, p2_c, thin, GREEN);
        line(g, p2_c, p3_b, thin, GREEN);

        let (p1_a2, p2_a2) = shift_line(shift_away * 2.0, p1, p2);
        let (p2_b2, p3_b2) = shift_line(shift_away * 2.0, p2, p3);
        let p2_c2 = line_intersection((p1_a2, p2_a2), (p2_b2, p3_b2));

        line(g, p1_a2, p2_c2, thin, GREEN);
        line(g, p2_c2, p3_b2, thin, GREEN);

        // Other side
        let (p1_e, p2_e) = shift_line(shift_away, p3, p2);
        let (p2_f, p3_f) = shift_line(shift_away, p2, p1);
        let p2_g = line_intersection((p1_e, p2_e), (p2_f, p3_f));

        line(g, p1_e, p2_g, thin, BLUE);
        line(g, p2_g, p3_f, thin, BLUE);

        //self.label(g, p1, &format!("p1 {:?}", p1));
        //self.label(g, p2, &format!("p2 {:?}", p2));
        self.label(g, p3, &format!("p3 {:?}", p3));
        /*self.label(g, p1_a, "p1_a");
        self.label(g, p2_a, "p2_a");
        self.label(g, p2_b, "p2_b");
        self.label(g, p3_b, "p3_b");
        self.label(g, p2_c, "p2_c");*/

        println!("");
        println!("p1 -> p2 is {}", angle_degrees(p1, p2));
        println!("p2 -> p3 is {}", angle_degrees(p2, p3));
    }
}

impl UI {
    fn label(&self, g: &mut GfxCtx, pt: (f64, f64), text: &str) {
        self.canvas
            .draw_text_at(g, &vec![text.to_string()], pt.0, pt.1);
    }
}

fn line(g: &mut GfxCtx, pt1: (f64, f64), pt2: (f64, f64), thickness: f64, color: Color) {
    let l = graphics::Line::new(color, thickness);
    l.draw(
        [pt1.0, pt1.1, pt2.0, pt2.1],
        &g.ctx.draw_state,
        g.ctx.transform,
        g.gfx,
    );
}

fn shift_line(width: f64, pt1: (f64, f64), pt2: (f64, f64)) -> ((f64, f64), (f64, f64)) {
    let x1 = pt1.0;
    let y1 = pt1.1;
    let x2 = pt2.0;
    let y2 = pt2.1;
    let half_pi = f64::consts::PI / 2.0;
    let angle = (y2 - y1).atan2(x2 - x1) + half_pi;
    let shifted1 = (x1 + width * angle.cos(), y1 + width * angle.sin());
    let shifted2 = (x2 + width * angle.cos(), y2 + width * angle.sin());
    (shifted1, shifted2)
}

fn angle_degrees(from: (f64, f64), to: (f64, f64)) -> f64 {
    // Y inversion necessary because of drawing
    let theta_rads = (from.1 - to.1).atan2(to.0 - from.0);
    let theta_degs = theta_rads * 360.0 / (2.0 * f64::consts::PI);
    // Normalize
    if theta_degs < 0.0 {
        theta_degs + 360.0
    } else {
        theta_degs
    }
}

// NOT segment. ignores parallel lines.
// https://en.wikipedia.org/wiki/Line%E2%80%93line_intersection#Given_two_points_on_each_line
fn line_intersection(l1: ((f64, f64), (f64, f64)), l2: ((f64, f64), (f64, f64))) -> (f64, f64) {
    let x1 = (l1.0).0;
    let y1 = (l1.0).1;
    let x2 = (l1.1).0;
    let y2 = (l1.1).1;

    let x3 = (l2.0).0;
    let y3 = (l2.0).1;
    let x4 = (l2.1).0;
    let y4 = (l2.1).1;

    let numer_x = (x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4);
    let numer_y = (x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4);
    let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
    (numer_x / denom, numer_y / denom)
}

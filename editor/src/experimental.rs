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
const BLACK: Color = [0.0, 0.0, 0.0, 0.3];

pub struct UI {
    canvas: Canvas,
    p3_offset: (f64, f64),
    show_labels: bool,
}

impl UI {
    pub fn new() -> UI {
        UI {
            canvas: Canvas::new(),
            p3_offset: (200.0, 150.0),
            show_labels: true,
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
        if input.unimportant_key_pressed(Key::P, "toggle labels") {
            self.show_labels = !self.show_labels;
        }

        self.canvas.handle_event(input.use_event_directly());

        (self, animation::EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, _input: UserInput) {
        graphics::clear(WHITE, g.gfx);
        g.ctx = self.canvas.get_transformed_context(&g.orig_ctx);

        let mut labels: Vec<((f64, f64), String)> = Vec::new();

        macro_rules! point {
            ($pt_name:ident, $value:expr) => {
                let $pt_name = $value;
                labels.push(($pt_name, stringify!($pt_name).to_string()));
            };
        }
        /*macro_rules! points {
            ($pt1_name:ident, $pt2_name:ident, $value:expr) => {
                let ($pt1_name, $pt2_name) = $value;
                labels.push(($pt1_name, stringify!($pt1_name).to_string()));
                labels.push(($pt2_name, stringify!($pt2_name).to_string()));
            };
        }*/

        let thin = 1.0;
        let thick = 5.0;
        let shift_away = 50.0;

        // TODO detect "breakages" by dist from p2 to p2_c beyond threshold
        // TODO still try the angle bisection method
        // TODO bezier curves could be ideal for both drawing and car paths, but no easy way to
        // try them out in piston

        point!(p1, (100.0, 100.0));
        point!(p2, (110.0, 200.0));
        point!(p3, (p1.0 + self.p3_offset.0, p1.1 + self.p3_offset.1));
        point!(p4, (500.0, 120.0));

        draw_polyline(g, vec![p1, p2, p3, p4], thick, RED);

        /*let polygon = polygon_for_polyline(&vec![p1, p2, p3, p4], shift_away);
        for (idx, pt) in polygon.iter().enumerate() {
            labels.push(((pt[0], pt[1]), format!("x{}", idx + 1)));
        }
        draw_polygon(g, polygon, BLACK);*/
        for p in polygons_for_polyline(&vec![p1, p2, p3, p4], shift_away) {
            draw_polygon(g, p, BLACK);
        }

        /*
        // Two lanes on one side of the road
        let l1_pts = shift_polyline(shift_away, &vec![p1, p2, p3, p4]);
        for (idx, pt) in l1_pts.iter().enumerate() {
            labels.push((*pt, format!("l1_p{}", idx + 1)));
        }
        draw_polyline(g, l1_pts, thin, GREEN);

        let l2_pts = shift_polyline(shift_away * 2.0, &vec![p1, p2, p3, p4]);
        for (idx, pt) in l2_pts.iter().enumerate() {
            labels.push((*pt, format!("l2_p{}", idx + 1)));
        }
        draw_polyline(g, l2_pts, thin, GREEN);

        // Other side
        let l3_pts = shift_polyline(shift_away, &vec![p4, p3, p2, p1]);
        for (idx, pt) in l3_pts.iter().enumerate() {
            labels.push((*pt, format!("l3_p{}", idx + 1)));
        }
        draw_polyline(g, l3_pts, thin, BLUE);
        */

        // Manual approach for more debugging
        /*points!(p1_e, p2_e, shift_line(shift_away, p3, p2));
        points!(p2_f, p3_f, shift_line(shift_away, p2, p1));
        point!(p2_g, line_intersection((p1_e, p2_e), (p2_f, p3_f)));

        draw_line(g, p1_e, p2_g, thin, BLUE);
        draw_line(g, p2_g, p3_f, thin, BLUE);*/

        if self.show_labels {
            for pair in &labels {
                self.label(g, pair.0, &pair.1);
            }
        }

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

fn draw_line(g: &mut GfxCtx, pt1: (f64, f64), pt2: (f64, f64), thickness: f64, color: Color) {
    let l = graphics::Line::new(color, thickness);
    l.draw(
        [pt1.0, pt1.1, pt2.0, pt2.1],
        &g.ctx.draw_state,
        g.ctx.transform,
        g.gfx,
    );
}

fn draw_polyline(g: &mut GfxCtx, pts: Vec<(f64, f64)>, thickness: f64, color: Color) {
    assert!(pts.len() >= 2);
    for pair in pts.windows(2) {
        draw_line(g, pair[0], pair[1], thickness, color);
    }
}

fn draw_polygon(g: &mut GfxCtx, pts: Vec<[f64; 2]>, color: Color) {
    graphics::Polygon::new(color).draw(&pts, &g.ctx.draw_state, g.ctx.transform, g.gfx);
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

// TODO unsure why this doesn't work. maybe see if mouse is inside polygon to check it out?
/*fn polygon_for_polyline(center_pts: &Vec<(f64, f64)>, width: f64) -> Vec<[f64; 2]> {
    let mut result = shift_polyline(width / 2.0, center_pts);
    let mut reversed_center_pts = center_pts.clone();
    reversed_center_pts.reverse();
    result.extend(shift_polyline(width / 2.0, &reversed_center_pts));
    // TODO unclear if piston needs last point to match the first or not
    let first_pt = result[0];
    result.push(first_pt);
    result.iter().map(|pair| [pair.0, pair.1]).collect()
}*/

// TODO why do we need a bunch of triangles? why doesn't the single polygon triangulate correctly?
// TODO ideally, detect when the polygon overlaps itself due to sharp lines and too much width
fn polygons_for_polyline(center_pts: &Vec<(f64, f64)>, width: f64) -> Vec<Vec<[f64; 2]>> {
    let side1 = shift_polyline(width / 2.0, center_pts);
    let mut reversed_center_pts = center_pts.clone();
    reversed_center_pts.reverse();
    let mut side2 = shift_polyline(width / 2.0, &reversed_center_pts);
    side2.reverse();

    let mut result: Vec<Vec<(f64, f64)>> = Vec::new();
    for high_idx in 1..center_pts.len() {
        result.push(vec![
            side1[high_idx],
            side1[high_idx - 1],
            side2[high_idx - 1],
        ]);
        result.push(vec![side2[high_idx], side2[high_idx - 1], side1[high_idx]]);
    }
    println!("{} triangles", result.len());
    result
        .iter()
        .map(|tri| tri.iter().map(|pair| [pair.0, pair.1]).collect())
        .collect()
}

fn shift_polyline(width: f64, pts: &Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    assert!(pts.len() >= 2);
    if pts.len() == 2 {
        let (pt1_shift, pt2_shift) = shift_line(width, pts[0], pts[1]);
        return vec![pt1_shift, pt2_shift];
    }

    let mut result: Vec<(f64, f64)> = Vec::new();

    let mut pt3_idx = 2;
    let mut pt1_raw = pts[0];
    let mut pt2_raw = pts[1];

    loop {
        let pt3_raw = pts[pt3_idx];

        let (pt1_shift, pt2_shift_1st) = shift_line(width, pt1_raw, pt2_raw);
        let (pt2_shift_2nd, pt3_shift) = shift_line(width, pt2_raw, pt3_raw);
        let pt2_shift = line_intersection((pt1_shift, pt2_shift_1st), (pt2_shift_2nd, pt3_shift));

        if pt3_idx == 2 {
            result.push(pt1_shift);
        }
        result.push(pt2_shift);
        if pt3_idx == pts.len() - 1 {
            result.push(pt3_shift);
            break;
        }

        pt1_raw = pt2_raw;
        pt2_raw = pt3_raw;
        pt3_idx += 1;
    }

    assert!(result.len() == pts.len());
    result
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

#[test]
fn shift_polyline_equivalence() {
    use rand;

    let scale = 1000.0;
    let pt1 = (rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt2 = (rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt3 = (rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt4 = (rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt5 = (rand::random::<f64>() * scale, rand::random::<f64>() * scale);

    let width = 50.0;
    let (pt1_s, _) = shift_line(width, pt1, pt2);
    let pt2_s = line_intersection(shift_line(width, pt1, pt2), shift_line(width, pt2, pt3));
    let pt3_s = line_intersection(shift_line(width, pt2, pt3), shift_line(width, pt3, pt4));
    let pt4_s = line_intersection(shift_line(width, pt3, pt4), shift_line(width, pt4, pt5));
    let (_, pt5_s) = shift_line(width, pt4, pt5);

    assert_eq!(
        shift_polyline(width, &vec![pt1, pt2, pt3, pt4, pt5]),
        vec![pt1_s, pt2_s, pt3_s, pt4_s, pt5_s]
    );
}

#[test]
fn shift_short_polyline_equivalence() {
    use rand;

    let scale = 1000.0;
    let pt1 = (rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt2 = (rand::random::<f64>() * scale, rand::random::<f64>() * scale);

    let width = 50.0;
    let (pt1_s, pt2_s) = shift_line(width, pt1, pt2);

    assert_eq!(shift_polyline(width, &vec![pt1, pt2]), vec![pt1_s, pt2_s]);
}

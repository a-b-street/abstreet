extern crate ezgui;
extern crate geom;
extern crate map_model;
extern crate piston;

use ezgui::{Canvas, Color, EventLoopMode, GfxCtx, Text, UserInput, GUI};
use geom::{Circle, PolyLine, Polygon, Pt2D};
use map_model::LANE_THICKNESS;
use piston::input::Key;
use std::f64;
use std::process;

const WHITE: Color = Color([1.0; 4]);
const RED: Color = Color([1.0, 0.0, 0.0, 0.8]);
const GREEN: Color = Color([0.0, 1.0, 0.0, 0.8]);
const BLUE: Color = Color([0.0, 0.0, 1.0, 0.8]);
const BLACK: Color = Color([0.0, 0.0, 0.0, 0.3]);
const SOLID_BLACK: Color = Color([0.0, 0.0, 0.0, 0.9]);
const YELLOW: Color = Color([1.0, 1.0, 0.0, 0.8]);

const KEY_CATEGORY: &str = "";

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

impl GUI for UI {
    fn event(&mut self, mut input: UserInput, _osd: &mut Text) -> EventLoopMode {
        if input.unimportant_key_pressed(Key::Escape, KEY_CATEGORY, "quit") {
            process::exit(0);
        }
        let speed = 5.0;
        if input.unimportant_key_pressed(Key::H, KEY_CATEGORY, "left") {
            self.p3_offset.0 -= speed;
        }
        if input.unimportant_key_pressed(Key::J, KEY_CATEGORY, "down") {
            self.p3_offset.1 += speed;
        }
        if input.unimportant_key_pressed(Key::K, KEY_CATEGORY, "up") {
            self.p3_offset.1 -= speed;
        }
        if input.unimportant_key_pressed(Key::L, KEY_CATEGORY, "right") {
            self.p3_offset.0 += speed;
        }
        if input.unimportant_key_pressed(Key::P, KEY_CATEGORY, "toggle labels") {
            self.show_labels = !self.show_labels;
        }
        if input.unimportant_key_pressed(Key::C, KEY_CATEGORY, "print current camera state") {
            println!("cam_zoom = {}", self.canvas.cam_zoom);
            println!(
                "center_on_map_pt({})",
                self.canvas.screen_to_map((
                    f64::from(self.canvas.window_size.width) / 2.0,
                    f64::from(self.canvas.window_size.height) / 2.0
                ))
            );
        }
        if input.unimportant_key_pressed(Key::D1, KEY_CATEGORY, "switch to mode 1") {
            self.current_mode = 1;
            self.canvas.cam_zoom = 1.0;
            self.canvas.center_on_map_pt(Pt2D::new(305.0, 324.0));
        }
        if input.unimportant_key_pressed(Key::D2, KEY_CATEGORY, "switch to mode 2") {
            self.current_mode = 2;
            self.canvas.cam_zoom = 10.0;
            self.canvas.center_on_map_pt(Pt2D::new(1352.0, 403.0));
        }
        if input.unimportant_key_pressed(Key::D3, KEY_CATEGORY, "switch to mode 3") {
            self.current_mode = 3;
            self.canvas.cam_zoom = 3.8;
            self.canvas.center_on_map_pt(Pt2D::new(2025.0, 1277.0));
        }
        if input.unimportant_key_pressed(Key::D4, KEY_CATEGORY, "switch to mode 4") {
            self.current_mode = 4;
            self.canvas.cam_zoom = 10.5;
            self.canvas.center_on_map_pt(Pt2D::new(122.0, 166.0));
        }
        if input.unimportant_key_pressed(Key::D5, KEY_CATEGORY, "switch to mode 5") {
            self.current_mode = 5;
            self.canvas.cam_zoom = 19.0;
            self.canvas.center_on_map_pt(Pt2D::new(1166.0, 766.0));
        }

        self.canvas.handle_event(&mut input);

        EventLoopMode::InputOnly
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, _osd: Text) {
        g.clear(WHITE);

        let mut labels: Vec<(Pt2D, String)> = Vec::new();

        match self.current_mode {
            1 => self.moving_polyline(g, &mut labels),
            2 => self.trim_polyline(g, &mut labels),
            3 => self.debug_intersection(g, &mut labels),
            4 => self.debug_polyline(g, &mut labels),
            5 => self.debug_polygon_drawing(g, &mut labels),
            x => panic!("Impossible current_mode {}", x),
        };

        // TODO detect "breakages" by dist from p2 to p2_c beyond threshold
        // TODO still try the angle bisection method
        // TODO bezier curves could be ideal for both drawing and car paths, but no easy way to
        // try them out in piston

        if self.show_labels {
            for pair in &labels {
                self.label(g, pair.0, &pair.1);
            }
        }
    }
}

impl UI {
    fn label(&self, g: &mut GfxCtx, pt: Pt2D, text: &str) {
        let mut txt = Text::new();
        txt.add_line(text.to_string());
        self.canvas.draw_text_at(g, txt, pt);
    }

    fn debug_polyline(&self, g: &mut GfxCtx, labels: &mut Vec<(Pt2D, String)>) {
        let thin = 1.0;
        let width = 50.0;

        // TODO retain this as a regression test
        let center_pts = PolyLine::new(
            vec![
                //Pt2D::new(2623.117354164207, 1156.9671270455774),
                //Pt2D::new(2623.0950086610856, 1162.8272397294127),
                Pt2D::new(2623.0956685132396, 1162.7341864981956),
                // One problem happens starting here -- some overlap
                Pt2D::new(2622.8995366939575, 1163.2433695162579),
                Pt2D::new(2620.4658232463926, 1163.9861244298272),
                Pt2D::new(2610.979416102837, 1164.2392149291984),
                //Pt2D::new(2572.5481805300115, 1164.2059309889344),
            ].iter()
            .map(|pt| Pt2D::new(pt.x() - 2500.0, pt.y() - 1000.0))
            .collect(),
        );
        draw_polyline(g, &center_pts, thin, RED);
        for (idx, pt) in center_pts.points().iter().enumerate() {
            labels.push((*pt, format!("p{}", idx + 1)));
        }

        if let Some(poly) = center_pts.make_polygons(width) {
            g.draw_polygon(BLACK, &poly);
        }

        // TODO colored labels!
        if let Some(side1) = center_pts.shift(width / 2.0) {
            //draw_polyline(g, &side1, thin, BLUE);
            for (idx, pt) in side1.points().iter().enumerate() {
                labels.push((*pt, format!("L{}", idx + 1)));
            }
        } else {
            println!("side1 borked");
        }

        if let Some(side2) = center_pts
            .reversed()
            .shift(width / 2.0)
            .map(|pl| pl.reversed())
        {
            //draw_polyline(g, &side2, thin, GREEN);
            for (idx, pt) in side2.points().iter().enumerate() {
                labels.push((*pt, format!("R{}", idx + 1)));
            }
        } else {
            println!("side2 borked");
        }
    }

    fn moving_polyline(&self, g: &mut GfxCtx, labels: &mut Vec<(Pt2D, String)>) {
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

        point!(p1, Pt2D::new(100.0, 100.0));
        point!(p2, Pt2D::new(110.0, 200.0));
        point!(
            p3,
            Pt2D::new(p1.x() + self.p3_offset.0, p1.y() + self.p3_offset.1)
        );
        point!(p4, Pt2D::new(500.0, 120.0));

        /*println!("");
        println!("p1 -> p2 is {}", p1.angle_to(p2));
        println!("p2 -> p3 is {}", p2.angle_to(p3));*/

        let pts = PolyLine::new(vec![p1, p2, p3, p4]);

        draw_polyline(g, &pts, thick, RED);

        if let Some(poly) = pts.make_polygons(shift_away) {
            g.draw_polygon(BLACK, &poly);
        }

        // Two lanes on one side of the road
        if let Some(l1_pts) = pts.shift(shift_away) {
            for (idx, pt) in l1_pts.points().iter().enumerate() {
                labels.push((*pt, format!("l1_p{}", idx + 1)));
            }
            draw_polyline(g, &l1_pts, thin, GREEN);
        } else {
            println!("l1_pts borked");
        }

        if let Some(l2_pts) = pts.shift(shift_away * 2.0) {
            for (idx, pt) in l2_pts.points().iter().enumerate() {
                labels.push((*pt, format!("l2_p{}", idx + 1)));
            }
            draw_polyline(g, &l2_pts, thin, GREEN);
        } else {
            println!("l2_pts borked");
        }

        // Other side
        if let Some(l3_pts) = pts.reversed().shift(shift_away) {
            for (idx, pt) in l3_pts.points().iter().enumerate() {
                labels.push((*pt, format!("l3_p{}", idx + 1)));
            }
            draw_polyline(g, &l3_pts, thin, BLUE);
        } else {
            println!("l3_pts borked");
        }
    }

    fn debug_intersection(&self, g: &mut GfxCtx, _labels: &mut Vec<(Pt2D, String)>) {
        let thin = 0.25;
        let shift1_width = LANE_THICKNESS * 0.5;
        let shift2_width = LANE_THICKNESS * 1.5;

        // All the center lines are expressed as incoming to the intersection
        let shared_pt = Pt2D::new(1983.3524141911557, 1260.9463599480669);
        let diagonal_yellow = PolyLine::new(vec![
            Pt2D::new(2165.2047110114004, 1394.0800456196182),
            shared_pt,
        ]);
        let north_yellow = PolyLine::new(vec![
            Pt2D::new(1983.7494225415583, 1187.7689787235172),
            Pt2D::new(1983.562154453436, 1221.9280601888336),
            shared_pt,
        ]);
        let south_yellow = PolyLine::new(vec![
            Pt2D::new(1979.8392648173965, 1345.9215228907012),
            Pt2D::new(1981.6744921024178, 1301.599225129214),
            Pt2D::new(1983.1876182714725, 1264.9938552786543),
            shared_pt,
        ]);

        for (yellow_line, colors) in &mut vec![
            (diagonal_yellow, RelatedColors::new(1.0, 0.0, 0.0)),
            (north_yellow, RelatedColors::new(0.0, 1.0, 0.0)),
            (south_yellow, RelatedColors::new(0.0, 0.0, 1.0)),
        ] {
            let lane1_in = yellow_line.shift(shift1_width).unwrap();
            draw_lane(g, &lane1_in, colors.next().unwrap());
            let lane2_in = yellow_line.shift(shift2_width).unwrap();
            draw_lane(g, &lane2_in, colors.next().unwrap());

            let lane1_out = yellow_line.reversed().shift(shift1_width).unwrap();
            draw_lane(g, &lane1_out, colors.next().unwrap());
            let lane2_out = yellow_line.reversed().shift(shift2_width).unwrap();
            draw_lane(g, &lane2_out, colors.next().unwrap());

            draw_polyline(g, &yellow_line, thin, YELLOW);
        }
    }

    fn trim_polyline(&self, g: &mut GfxCtx, _labels: &mut Vec<(Pt2D, String)>) {
        let mut vertical_pl = PolyLine::new(vec![
            Pt2D::new(1333.9512635794777, 413.3946082988369),
            Pt2D::new(1333.994382315137, 412.98183477602896),
            Pt2D::new(1334.842742789155, 408.38697863519786),
            Pt2D::new(1341.8334675664184, 388.5049183955915),
            Pt2D::new(1343.4401359706367, 378.05011956849677),
            Pt2D::new(1344.2823018114202, 367.36774792310285),
        ]).reversed();
        let mut horiz_pl = PolyLine::new(vec![
            Pt2D::new(1388.995635038006, 411.7906956729764),
            Pt2D::new(1327.388582742321, 410.78740100896965),
        ]);

        let hit = vertical_pl.intersection(&horiz_pl).unwrap();
        if false {
            g.draw_circle(BLUE, &Circle::new(hit, 1.0));
        } else {
            vertical_pl.trim_to_pt(hit);
            horiz_pl.trim_to_pt(hit);
        }

        draw_polyline(g, &vertical_pl, 0.25, RED);
        draw_polyline(g, &horiz_pl, 0.25, GREEN);
    }

    fn debug_polygon_drawing(&self, g: &mut GfxCtx, labels: &mut Vec<(Pt2D, String)>) {
        let pts = vec![
            Pt2D::new(1158.5480421283125, 759.4168710122531), // 0
            Pt2D::new(1158.3757450502824, 776.1517074719404), // 1
            Pt2D::new(1174.6840382119703, 776.3184998618594), // 2
            Pt2D::new(1174.3469352293675, 759.4168710122531), // 3
            Pt2D::new(1158.5480421283125, 759.4168710122531), // 4
        ];
        //draw_polyline(g, &PolyLine::new(pts.clone()), 0.25, RED);
        g.draw_polygon(BLUE, &Polygon::new(&pts));

        for (idx, pt) in pts.iter().enumerate() {
            labels.push((*pt, format!("{}", idx)));
        }
    }
}

fn draw_polyline(g: &mut GfxCtx, pl: &PolyLine, thickness: f64, color: Color) {
    for l in pl.lines() {
        g.draw_line(color, thickness, &l);
    }
    let radius = 0.5;
    let pts = pl.points();
    assert!(pts.len() >= 2);
    for pt in pts {
        g.draw_circle(BLUE, &Circle::new(*pt, radius));
    }
}

fn draw_lane(g: &mut GfxCtx, pl: &PolyLine, color: Color) {
    g.draw_polygon(color, &pl.make_polygons(LANE_THICKNESS).unwrap());

    // Debug the center points
    draw_polyline(g, pl, 0.25, SOLID_BLACK);
}

struct RelatedColors {
    r: f32,
    g: f32,
    b: f32,
    count: usize,
}

impl RelatedColors {
    fn new(r: f32, g: f32, b: f32) -> RelatedColors {
        RelatedColors { r, g, b, count: 10 }
    }
}

impl Iterator for RelatedColors {
    type Item = Color;

    fn next(&mut self) -> Option<Color> {
        self.count -= 2;
        let multiplier = 0.1 * (self.count as f32);
        Some(Color([
            self.r * multiplier,
            self.g * multiplier,
            self.b * multiplier,
            0.8,
        ]))
    }
}

fn main() {
    ezgui::run(UI::new(), "GUI Playground", 1024, 768);
}

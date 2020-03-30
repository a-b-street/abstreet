use geom::{Line, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Color {
    RGBA(f32, f32, f32, f32),
    // TODO Figure out how to pack more data into this.
    HatchingStyle1,
    HatchingStyle2,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Color::RGBA(r, g, b, a) => write!(f, "Color(r={}, g={}, b={}, a={})", r, g, b, a),
            Color::HatchingStyle1 => write!(f, "Color::HatchingStyle1"),
            Color::HatchingStyle2 => write!(f, "Color::HatchingStyle2"),
        }
    }
}

// TODO Not sure if this is hacky or not. Maybe Color should be specialized to RGBA, and these are
// other cases...
#[derive(Clone, PartialEq)]
pub enum FancyColor {
    Plain(Color),
    // The line, then stops (percent along, color)
    LinearGradient(Line, Vec<(f64, Color)>),
}

impl Color {
    // TODO Won't this confuse the shader? :P
    pub const INVISIBLE: Color = Color::rgba_f(1.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::rgb_f(0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgb_f(1.0, 1.0, 1.0);
    pub const RED: Color = Color::rgb_f(1.0, 0.0, 0.0);
    pub const GREEN: Color = Color::rgb_f(0.0, 1.0, 0.0);
    pub const BLUE: Color = Color::rgb_f(0.0, 0.0, 1.0);
    pub const CYAN: Color = Color::rgb_f(0.0, 1.0, 1.0);
    pub const YELLOW: Color = Color::rgb_f(1.0, 1.0, 0.0);
    pub const PURPLE: Color = Color::rgb_f(0.5, 0.0, 0.5);
    pub const PINK: Color = Color::rgb_f(1.0, 0.41, 0.71);
    pub const ORANGE: Color = Color::rgb_f(1.0, 0.55, 0.0);

    // TODO should assert stuff about the inputs

    // TODO Once f32 can be used in const fn, make these const fn too and clean up call sites
    // dividing by 255.0. https://github.com/rust-lang/rust/issues/57241
    pub fn rgb(r: usize, g: usize, b: usize) -> Color {
        Color::rgba(r, g, b, 1.0)
    }

    pub const fn rgb_f(r: f32, g: f32, b: f32) -> Color {
        Color::RGBA(r, g, b, 1.0)
    }

    pub fn rgba(r: usize, g: usize, b: usize, a: f32) -> Color {
        Color::RGBA(
            (r as f32) / 255.0,
            (g as f32) / 255.0,
            (b as f32) / 255.0,
            a,
        )
    }

    pub const fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color::RGBA(r, g, b, a)
    }

    pub const fn grey(f: f32) -> Color {
        Color::RGBA(f, f, f, 1.0)
    }

    pub fn alpha(&self, a: f32) -> Color {
        match self {
            Color::RGBA(r, g, b, _) => Color::RGBA(*r, *g, *b, a),
            _ => unreachable!(),
        }
    }

    pub fn fade(&self, factor: f32) -> Color {
        match self {
            Color::RGBA(r, g, b, a) => Color::RGBA(*r / factor, *g / factor, *b / factor, *a),
            _ => unreachable!(),
        }
    }

    pub fn hex(raw: &str) -> Color {
        // Skip the leading '#'
        let r = usize::from_str_radix(&raw[1..3], 16).unwrap();
        let g = usize::from_str_radix(&raw[3..5], 16).unwrap();
        let b = usize::from_str_radix(&raw[5..7], 16).unwrap();
        Color::rgb(r, g, b)
    }

    pub fn to_hex(&self) -> String {
        match self {
            Color::RGBA(r, g, b, _) => format!(
                "#{:02X}{:02X}{:02X}",
                (r * 255.0) as usize,
                (g * 255.0) as usize,
                (b * 255.0) as usize
            ),
            _ => unreachable!(),
        }
    }

    fn lerp(self, other: Color, pct: f32) -> Color {
        match (self, other) {
            (Color::RGBA(r1, g1, b1, a1), Color::RGBA(r2, g2, b2, a2)) => Color::RGBA(
                lerp(pct, (r1, r2)),
                lerp(pct, (g1, g2)),
                lerp(pct, (b1, b2)),
                lerp(pct, (a1, a2)),
            ),
            _ => unreachable!(),
        }
    }
}

impl FancyColor {
    pub(crate) fn linear_gradient(lg: &usvg::LinearGradient) -> FancyColor {
        let line = Line::new(Pt2D::new(lg.x1, lg.y1), Pt2D::new(lg.x2, lg.y2));
        let mut stops = Vec::new();
        for stop in &lg.stops {
            let color = Color::rgba(
                stop.color.red as usize,
                stop.color.green as usize,
                stop.color.blue as usize,
                stop.opacity.value() as f32,
            );
            stops.push((stop.offset.value(), color));
        }
        FancyColor::LinearGradient(line, stops)
    }

    pub(crate) fn interp_lg(line: &Line, stops: &Vec<(f64, Color)>, corner: Pt2D) -> Color {
        // https://developer.mozilla.org/en-US/docs/Web/CSS/linear-gradient is the best reference
        // I've found, even though it's technically for CSS, not SVG
        let pct = line
            .percent_along_of_point(line.project_pt(corner))
            .unwrap();
        if pct < stops[0].0 {
            return stops[0].1;
        }
        if pct > stops.last().unwrap().0 {
            return stops.last().unwrap().1;
        }
        // In between two
        for ((pct1, c1), (pct2, c2)) in stops.iter().zip(stops.iter().skip(1)) {
            if pct >= *pct1 && pct <= *pct2 {
                return c1.lerp(*c2, to_pct(pct, (*pct1, *pct2)) as f32);
            }
        }
        unreachable!()
    }
}

fn to_pct(value: f64, (low, high): (f64, f64)) -> f64 {
    assert!(low <= high);
    assert!(value >= low);
    assert!(value <= high);
    (value - low) / (high - low)
}

fn lerp(pct: f32, (x1, x2): (f32, f32)) -> f32 {
    x1 + pct * (x2 - x1)
}

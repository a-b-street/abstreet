use geom::{Line, Pt2D};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Color(r={}, g={}, b={}, a={})",
            self.r, self.g, self.b, self.a
        )
    }
}

// TODO Maybe needs a better name
#[derive(Clone, PartialEq)]
pub enum FancyColor {
    RGBA(Color),
    Hatching,
    LinearGradient(LinearGradient),
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
        Color { r, g, b, a: 1.0 }
    }

    pub fn rgba(r: usize, g: usize, b: usize, a: f32) -> Color {
        Color {
            r: (r as f32) / 255.0,
            g: (g as f32) / 255.0,
            b: (b as f32) / 255.0,
            a,
        }
    }

    pub const fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    pub const fn grey(f: f32) -> Color {
        Color::rgb_f(f, f, f)
    }

    pub const fn alpha(&self, a: f32) -> Color {
        Color::rgba_f(self.r, self.g, self.b, a)
    }

    pub fn fade(&self, factor: f32) -> Color {
        let mut c = self.clone();
        c.r /= factor;
        c.g /= factor;
        c.b /= factor;
        c
    }

    pub fn hex(raw: &str) -> Color {
        // Skip the leading '#'
        let r = usize::from_str_radix(&raw[1..3], 16).unwrap();
        let g = usize::from_str_radix(&raw[3..5], 16).unwrap();
        let b = usize::from_str_radix(&raw[5..7], 16).unwrap();
        Color::rgb(r, g, b)
    }

    pub fn to_hex(&self) -> String {
        format!(
            "#{:02X}{:02X}{:02X}",
            (self.r * 255.0) as usize,
            (self.g * 255.0) as usize,
            (self.b * 255.0) as usize
        )
    }

    fn lerp(self, other: Color, pct: f32) -> Color {
        Color::rgba_f(
            lerp(pct, (self.r, other.r)),
            lerp(pct, (self.g, other.g)),
            lerp(pct, (self.b, other.b)),
            lerp(pct, (self.a, other.a)),
        )
    }
}

// https://developer.mozilla.org/en-US/docs/Web/CSS/linear-gradient is the best reference I've
// found, even though it's technically for CSS, not SVG. Ah, and
// https://www.w3.org/TR/SVG11/pservers.html
#[derive(Clone, PartialEq)]
pub struct LinearGradient {
    line: Line,
    stops: Vec<(f64, Color)>,
}

impl LinearGradient {
    pub(crate) fn new(lg: &usvg::LinearGradient) -> FancyColor {
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
        FancyColor::LinearGradient(LinearGradient { line, stops })
    }

    fn interp(&self, pt: Pt2D) -> Color {
        let pct = self
            .line
            .percent_along_of_point(self.line.project_pt(pt))
            .unwrap();
        if pct < self.stops[0].0 {
            return self.stops[0].1;
        }
        if pct > self.stops.last().unwrap().0 {
            return self.stops.last().unwrap().1;
        }
        // In between two
        for ((pct1, c1), (pct2, c2)) in self.stops.iter().zip(self.stops.iter().skip(1)) {
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

impl FancyColor {
    pub(crate) fn style(&self, pt: Pt2D) -> [f32; 4] {
        match self {
            FancyColor::RGBA(c) => [c.r, c.g, c.b, c.a],
            FancyColor::Hatching => [100.0, 0.0, 0.0, 0.0],
            FancyColor::LinearGradient(ref lg) => {
                let c = lg.interp(pt);
                [c.r, c.g, c.b, c.a]
            }
        }
    }
}

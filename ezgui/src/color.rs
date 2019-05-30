use serde_derive::{Deserialize, Serialize};
use std::fmt;

// Copy could be reconsidered, but eh
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color(pub(crate) [f32; 4]);

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Color(r={}, g={}, b={}, a={})",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl Color {
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
    // dividing by 255.0
    pub fn rgb(r: usize, g: usize, b: usize) -> Color {
        Color::rgba(r, g, b, 1.0)
    }

    pub const fn rgb_f(r: f32, g: f32, b: f32) -> Color {
        Color([r, g, b, 1.0])
    }

    pub fn rgba(r: usize, g: usize, b: usize, a: f32) -> Color {
        Color([
            (r as f32) / 255.0,
            (g as f32) / 255.0,
            (b as f32) / 255.0,
            a,
        ])
    }

    pub const fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color([r, g, b, a])
    }

    pub const fn grey(f: f32) -> Color {
        Color([f, f, f, 1.0])
    }

    pub const fn alpha(&self, a: f32) -> Color {
        Color([self.0[0], self.0[1], self.0[2], a])
    }

    pub fn from_string(color: &str) -> Color {
        match color {
            "red" => Color::RED,
            "cyan" => Color::CYAN,
            _ => panic!("Unknown color {}", color),
        }
    }

    pub fn to_string(self) -> &'static str {
        if self == Color::RED {
            return "red";
        }
        if self == Color::CYAN {
            return "cyan";
        }
        panic!("Can't transform {} to a string", self);
    }
}

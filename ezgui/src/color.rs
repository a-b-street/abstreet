use crate::ScreenDims;
use geom::Angle;
use serde_derive::{Deserialize, Serialize};
use std::fmt;

// Group index, texture index
type TextureID = (f32, f32);

// Copy could be reconsidered, but eh
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Color {
    RGBA(f32, f32, f32, f32),
    // Tiles seamlessly through all of map-space.
    TileTexture(TextureID, ScreenDims),
    // Stretches the entire texture to fit the entire polygon. Rotates from the center of the
    // polygon. Not sure what this means for anything but circles right now. Have to manually
    // fiddle with the original orientation to fix y inversion.
    StretchTexture(TextureID, ScreenDims, Angle),
    // TODO Figure out how to pack more data into this.
    HatchingStyle1,
    HatchingStyle2,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Color::RGBA(r, g, b, a) => write!(f, "Color(r={}, g={}, b={}, a={})", r, g, b, a),
            Color::TileTexture(id, dims) => write!(
                f,
                "Color::TileTexture({}:{}, width={}, height={})",
                id.0, id.1, dims.width, dims.height
            ),
            Color::StretchTexture(id, dims, angle) => write!(
                f,
                "Color::StretchTexture({}:{}, width={}, height={}, {})",
                id.0, id.1, dims.width, dims.height, angle
            ),
            Color::HatchingStyle1 => write!(f, "Color::HatchingStyle1"),
            Color::HatchingStyle2 => write!(f, "Color::HatchingStyle2"),
        }
    }
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

    pub fn rotate(&self, angle: Angle) -> Color {
        match self {
            Color::StretchTexture(id, dims, _) => Color::StretchTexture(*id, *dims, angle),
            _ => unreachable!(),
        }
    }

    pub fn texture_dims(&self) -> ScreenDims {
        match self {
            Color::TileTexture(_, dims) => *dims,
            Color::StretchTexture(_, dims, _) => *dims,
            _ => unreachable!(),
        }
    }
}

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

mod building;
mod car;
mod extra_shape;
mod intersection;
mod lane;
mod map;
mod parcel;
mod pedestrian;
mod turn;

use aabb_quadtree::geom::{Point, Rect};
use ezgui::GfxCtx;
use geom::{Bounds, Pt2D};
use graphics::types::Color;
use map_model::{geometry, Map};
use objects::{Ctx, ID};
pub use render::car::DrawCar;
pub use render::lane::DrawLane;
pub use render::map::DrawMap;
pub use render::pedestrian::DrawPedestrian;
pub use render::turn::DrawTurn;
use std::f64;

// These are all in meters
const PARCEL_BOUNDARY_THICKNESS: f64 = 0.5;
const BUILDING_BOUNDARY_THICKNESS: f64 = 1.5;
const EXTRA_SHAPE_THICKNESS: f64 = 1.0;
const EXTRA_SHAPE_POINT_RADIUS: f64 = 1.0;

const TURN_ICON_ARROW_THICKNESS: f64 = geometry::BIG_ARROW_THICKNESS / 3.0;
const BIG_ARROW_TIP_LENGTH: f64 = 1.0;
const TURN_ICON_ARROW_TIP_LENGTH: f64 = BIG_ARROW_TIP_LENGTH * 0.8;
const TURN_ICON_ARROW_LENGTH: f64 = 2.0;

pub fn get_bbox(b: &Bounds) -> Rect {
    Rect {
        top_left: Point {
            x: b.min_x as f32,
            y: b.min_y as f32,
        },
        bottom_right: Point {
            x: b.max_x as f32,
            y: b.max_y as f32,
        },
    }
}

pub trait Renderable {
    fn get_id(&self) -> ID;
    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx);
    // TODO Maybe return Bounds
    fn get_bbox(&self) -> Rect;
    fn contains_pt(&self, pt: Pt2D) -> bool;
    fn tooltip_lines(&self, map: &Map) -> Vec<String>;
}

pub struct RenderOptions {
    // The "main" color for the object, whatever that means
    // TODO Building needs two colors
    // TODO maybe each renderable should decide color logic, using the colorscheme. pass in info
    // from other plugins like 'selected?'
    pub color: Color,
    pub cam_zoom: f64,
    pub debug_mode: bool,
}

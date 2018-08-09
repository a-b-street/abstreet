// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

mod building;
mod intersection;
mod lane;
mod map;
mod parcel;
mod turn;

use aabb_quadtree::geom::{Point, Rect};
use geom::Bounds;
use map_model::geometry;
pub use render::lane::DrawLane;
pub use render::map::DrawMap;
pub use render::turn::DrawTurn;
use std::f64;

// These are all in meters
const PARCEL_BOUNDARY_THICKNESS: f64 = 0.5;

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

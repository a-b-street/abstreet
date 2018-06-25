// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

mod building;
mod intersection;
mod map;
mod parcel;
mod road;
mod turn;

use map_model::geometry;
pub use render::map::DrawMap;
pub use render::road::DrawRoad;
pub use render::turn::DrawTurn;
use std::f64;

// These are all in meters
const PARCEL_BOUNDARY_THICKNESS: f64 = 0.5;

const TURN_ICON_ARROW_THICKNESS: f64 = geometry::BIG_ARROW_THICKNESS / 3.0;
const BIG_ARROW_TIP_LENGTH: f64 = 1.0;
const TURN_ICON_ARROW_TIP_LENGTH: f64 = BIG_ARROW_TIP_LENGTH * 0.8;
const TURN_ICON_ARROW_LENGTH: f64 = 2.0;

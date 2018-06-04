// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate map_model;

mod building;
mod intersection;
mod map;
mod parcel;
mod road;
mod turn;

use ezgui::canvas;
use geom;
use graphics::types::Color;
pub use render::map::DrawMap;
pub use render::road::DrawRoad;
pub use render::turn::DrawTurn;
use std::f64;

// These are all in meters
const PARCEL_BOUNDARY_THICKNESS: f64 = 0.5;

const TURN_ICON_ARROW_THICKNESS: f64 = geom::BIG_ARROW_THICKNESS / 3.0;
const BIG_ARROW_TIP_LENGTH: f64 = 1.0;
const TURN_ICON_ARROW_TIP_LENGTH: f64 = BIG_ARROW_TIP_LENGTH * 0.8;
const TURN_ICON_ARROW_LENGTH: f64 = 2.0;

pub const DEBUG_COLOR: Color = canvas::PURPLE;
pub const BRIGHT_DEBUG_COLOR: Color = [1.0, 0.1, 0.55, 1.0];
pub const ROAD_COLOR: Color = canvas::BLACK;
pub const CHANGED_STOP_SIGN_INTERSECTION_COLOR: Color = canvas::GREEN;
pub const CHANGED_TRAFFIC_SIGNAL_INTERSECTION_COLOR: Color = canvas::ORANGE;
pub const TRAFFIC_SIGNAL_INTERSECTION_COLOR: Color = canvas::YELLOW;
pub const NORMAL_INTERSECTION_COLOR: Color = canvas::DARK_GREY;
pub const SELECTED_COLOR: Color = canvas::BLUE;
pub const TURN_COLOR: Color = canvas::GREEN;
pub const CONFLICTING_TURN_COLOR: Color = [1.0, 0.0, 0.0, 0.5];
pub const BUILDING_COLOR: Color = canvas::LIGHT_GREY;
pub const PARCEL_COLOR: Color = canvas::DARK_GREY;
const ROAD_ORIENTATION_COLOR: Color = canvas::YELLOW;
pub const SEARCH_RESULT_COLOR: Color = canvas::RED;
// For interactive algorithms
pub const VISITED_COLOR: Color = canvas::BLUE;
pub const QUEUED_COLOR: Color = canvas::RED;
pub const NEXT_QUEUED_COLOR: Color = canvas::GREEN;
const TURN_ICON_CIRCLE_COLOR: Color = canvas::DARK_GREY;
pub const TURN_ICON_INACTIVE_COLOR: Color = canvas::LIGHT_GREY;

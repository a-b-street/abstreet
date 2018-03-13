// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate map_model;

mod building;
mod intersection;
mod map;
mod parcel;
mod road;
mod turn;

use ezgui::canvas;
use graphics::types::Color;
pub use render::map::DrawMap;
pub use render::road::DrawRoad;
pub use render::turn::DrawTurn;
use std::f64;

// These are all in meters
const PARCEL_BOUNDARY_THICKNESS: f64 = 0.5;
const LANE_THICKNESS: f64 = 2.5;

const BIG_ARROW_THICKNESS: f64 = 0.5;
const TURN_ICON_ARROW_THICKNESS: f64 = BIG_ARROW_THICKNESS / 3.0;
const BIG_ARROW_TIP_LENGTH: f64 = 1.0;
const TURN_ICON_ARROW_TIP_LENGTH: f64 = BIG_ARROW_TIP_LENGTH * 0.8;
const TURN_ICON_ARROW_LENGTH: f64 = 2.0;

const TURN_DIST_FROM_INTERSECTION: f64 = 7.5;

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

// TODO I don't think this is a useful trait at all. The input here really just depends. All the
// logic winds up happening in ui anyway.
pub trait ColorChooser {
    fn color_r(&self, _: &map_model::Road) -> Option<Color> {
        None
    }
    fn color_i(&self, _: &map_model::Intersection) -> Option<Color> {
        None
    }
    fn color_t(&self, _: &map_model::Turn) -> Option<Color> {
        None
    }
    fn color_b(&self, _: &map_model::Building) -> Option<Color> {
        None
    }
    fn color_p(&self, _: &map_model::Parcel) -> Option<Color> {
        None
    }
}

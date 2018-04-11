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

// TODO maybe use dimensioned way more thoroughly inside this crate

extern crate aabb_quadtree;
#[macro_use]
extern crate dimensioned;
extern crate graphics;
extern crate map_model;
extern crate vecmath;

pub mod geometry;
mod map;
mod road;
mod turn;

pub use geometry::angles::{Radian, RAD};
pub use map::GeomMap;
pub use road::GeomRoad;
pub use turn::GeomTurn;

pub const LANE_THICKNESS: f64 = 2.5;
pub const BIG_ARROW_THICKNESS: f64 = 0.5;
pub const TURN_DIST_FROM_INTERSECTION: f64 = 7.5;

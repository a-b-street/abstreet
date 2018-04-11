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

use ezgui::canvas;
use graphics::types::Color;
use map_model;

// TODO have some UI for editing these rules and saving them
pub struct OsmClassifier {}

impl OsmClassifier {
    pub fn color_r(&self, r: &map_model::Road) -> Option<Color> {
        for tag in &r.osm_tags {
            if tag == "highway=primary" || tag == "highway=secondary" || tag == "highway=tertiary" {
                return Some(canvas::GREEN);
            }
        }
        Some(canvas::ALMOST_INVISIBLE)
    }
    pub fn color_b(&self, b: &map_model::Building) -> Option<Color> {
        for tag in &b.osm_tags {
            if tag.contains("addr:housenumber") {
                return Some(canvas::RED);
            }
        }
        None
    }
}

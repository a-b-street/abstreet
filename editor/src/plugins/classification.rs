// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use graphics::types::Color;
use map_model;

// TODO have some UI for editing these rules and saving them
pub struct OsmClassifier {}

impl OsmClassifier {
    pub fn color_r(&self, r: &map_model::Road, cs: &ColorScheme) -> Option<Color> {
        for tag in &r.osm_tags {
            if tag == "highway=primary" || tag == "highway=secondary" || tag == "highway=tertiary" {
                return Some(cs.get(Colors::MatchClassification));
            }
        }
        Some(cs.get(Colors::DontMatchClassification))
    }
    pub fn color_b(&self, b: &map_model::Building, cs: &ColorScheme) -> Option<Color> {
        for tag in &b.osm_tags {
            if tag.contains("addr:housenumber") {
                return Some(cs.get(Colors::MatchClassification));
            }
        }
        None
    }
}

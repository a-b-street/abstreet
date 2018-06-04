// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

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

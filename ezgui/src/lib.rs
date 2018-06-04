// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate graphics;
extern crate opengl_graphics;
extern crate piston;

pub mod canvas;
pub mod input;
pub mod text_box;

use piston::input::Key;

pub struct ToggleableLayer {
    layer_name: String,
    key: Key,
    key_name: String,
    // If None, never automatically enable at a certain zoom level.
    min_zoom: Option<f64>,

    enabled: bool,
}

impl ToggleableLayer {
    pub fn new(
        layer_name: &str,
        key: Key,
        key_name: &str,
        min_zoom: Option<f64>,
    ) -> ToggleableLayer {
        ToggleableLayer {
            key,
            min_zoom,
            layer_name: String::from(layer_name),
            key_name: String::from(key_name),
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn handle_zoom(&mut self, before_zoom: f64, after_zoom: f64) {
        if let Some(threshold) = self.min_zoom {
            let before_value = before_zoom >= threshold;
            let after_value = after_zoom >= threshold;
            if before_value != after_value {
                self.enabled = after_value;
            }
        }
    }

    pub fn handle_event(&mut self, input: &mut input::UserInput) -> bool {
        if input.unimportant_key_pressed(
            self.key,
            &format!("Press {} to toggle {}", self.key_name, self.layer_name),
        ) {
            self.enabled = !self.enabled;
            return true;
        }
        false
    }
}

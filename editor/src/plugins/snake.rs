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

use animation;
use ezgui::canvas::{Canvas, GfxCtx};
use ezgui::input::UserInput;
use graphics::types::Color;
use map_model::{Map, Pt2D, Road, RoadID, TurnID};
use piston::input::{Key, UpdateEvent};
use piston::window::Size;
use render;
use std::collections::HashSet;

// TODO consider speed instead, so we dont always pan so slowly
const PAN_TIME_S: f64 = 1.0;

pub struct Snake {
    current: RoadID,
    visited: HashSet<RoadID>,

    center_camera: Option<animation::LineLerp>,
}

impl Snake {
    pub fn new(start: RoadID) -> Snake {
        let mut s = Snake {
            current: start,
            visited: HashSet::new(),
            // TODO do this when we first start the game
            center_camera: None,
        };
        s.visited.insert(start);
        s
    }

    pub fn draw(&self, map: &Map, canvas: &Canvas, draw_map: &render::DrawMap, g: &mut GfxCtx) {
        for (idx, turn) in self.get_valid_moves(map).iter().enumerate() {
            let t = draw_map.get_t(*turn);
            t.draw_full(g, render::TURN_COLOR);
            canvas.draw_text_at(g, &vec![(idx + 1).to_string()], t.dst_pt[0], t.dst_pt[1]);
        }
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        draw_map: &render::DrawMap,
        canvas: &mut Canvas,
        window_size: &Size,
    ) -> bool {
        if input.use_event_directly().update_args().is_some() {
            let done = if let Some(ref c) = self.center_camera {
                let pt = c.get_pt();
                canvas.center_on_map_pt(pt.x(), pt.y(), window_size);
                c.lerp.is_done()
            } else {
                false
            };
            if done {
                self.center_camera = None;
            }
        }

        // Exit game
        if input.key_pressed(Key::Return, "Press enter to quit Snake") {
            return true;
        }

        let moves = self.get_valid_moves(map);
        if let Some(n) = input.number_chosen(
            moves.len(),
            &format!("Press 1 - {} to select a move", moves.len()),
        ) {
            let dst = map.get_t(moves[n - 1]).dst;
            self.current = dst;
            self.visited.insert(dst);

            let at = canvas.get_cursor_in_map_space();
            let center = draw_map
                .get_i(map.get_destination_intersection(dst).id)
                .point;
            self.center_camera = Some(animation::LineLerp {
                from: Pt2D::new(at.0, at.1),
                to: Pt2D::new(center[0], center[1]),
                lerp: animation::TimeLerp::with_dur_s(PAN_TIME_S),
            });
        }

        false
    }

    fn get_valid_moves(&self, map: &Map) -> Vec<TurnID> {
        map.get_turns_from_road(self.current)
            .iter()
            .filter(|t| !self.visited.contains(&t.dst))
            .map(|t| t.id)
            .collect()
    }

    pub fn color_r(&self, r: &Road) -> Option<Color> {
        if self.current == r.id {
            return Some(render::NEXT_QUEUED_COLOR);
        }
        if self.visited.contains(&r.id) {
            return Some(render::VISITED_COLOR);
        }
        None
    }
}

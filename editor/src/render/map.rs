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

extern crate aabb_quadtree;
extern crate map_model;

use aabb_quadtree::QuadTree;
use aabb_quadtree::geom::{Point, Rect};
use geometry;
use map_model::{Bounds, BuildingID, IntersectionID, Map, ParcelID, Pt2D, RoadID, TurnID};
use render::building::DrawBuilding;
use render::intersection::DrawIntersection;
use render::parcel::DrawParcel;
use render::road::DrawRoad;
use render::turn::DrawTurn;
use std::collections::HashMap;

pub struct DrawMap {
    pub roads: Vec<DrawRoad>,
    pub intersections: Vec<DrawIntersection>,
    pub turns: Vec<DrawTurn>,
    pub buildings: Vec<DrawBuilding>,
    pub parcels: Vec<DrawParcel>,

    roads_quadtree: QuadTree<RoadID>,
    road_icons_quadtree: QuadTree<RoadID>,
    intersections_quadtree: QuadTree<IntersectionID>,
    turn_icons_quadtree: QuadTree<TurnID>,
    buildings_quadtree: QuadTree<BuildingID>,
    parcels_quadtree: QuadTree<ParcelID>,
}

impl DrawMap {
    // Also returns the center of the map in map-space and the max pt in screen-space
    pub fn new(map: &Map) -> (DrawMap, Bounds, Pt2D, Pt2D) {
        let bounds = map.get_gps_bounds();

        let mut roads: Vec<DrawRoad> = Vec::new();
        for r in map.all_roads() {
            let leads_to_stop_sign = !map.get_destination_intersection(r.id).has_traffic_signal;
            roads.push(DrawRoad::new(r, &bounds, leads_to_stop_sign));
        }

        let mut turn_to_road_offset: HashMap<TurnID, usize> = HashMap::new();
        for r in map.all_roads() {
            let mut turns = map.get_turns_from_road(r.id);
            // Sort the turn icons by angle.
            turns.sort_by_key(|t| {
                let src_pt = roads[t.src.0].last_pt();
                let dst_pt = roads[t.dst.0].first_pt();
                let mut angle = (dst_pt[1] - src_pt[1])
                    .atan2(dst_pt[0] - src_pt[0])
                    .to_degrees();
                if angle < 0.0 {
                    angle += 360.0;
                }
                angle as i64
            });

            for (idx, t) in turns.iter().enumerate() {
                turn_to_road_offset.insert(t.id, idx);
            }
        }

        let turns: Vec<DrawTurn> = map.all_turns()
            .iter()
            .map(|t| {
                DrawTurn::new(
                    &roads,
                    t,
                    turn_to_road_offset[&t.id],
                    !map.get_i(t.parent).has_traffic_signal,
                )
            })
            .collect();
        let intersections: Vec<DrawIntersection> = map.all_intersections()
            .iter()
            .map(|i| DrawIntersection::new(i, &map, &roads, &bounds))
            .collect();
        let buildings: Vec<DrawBuilding> = map.all_buildings()
            .iter()
            .map(|b| DrawBuilding::new(b, &bounds))
            .collect();
        let parcels: Vec<DrawParcel> = map.all_parcels()
            .iter()
            .map(|p| DrawParcel::new(p, &bounds))
            .collect();

        // min_y here due to the wacky y inversion
        let max_screen_pt =
            geometry::gps_to_screen_space(&Pt2D::new(bounds.max_x, bounds.min_y), &bounds);
        let map_bbox = Rect {
            top_left: Point { x: 0.0, y: 0.0 },
            bottom_right: Point {
                x: max_screen_pt.x() as f32,
                y: max_screen_pt.y() as f32,
            },
        };

        let mut roads_quadtree = QuadTree::default(map_bbox);
        let mut road_icons_quadtree = QuadTree::default(map_bbox);
        for r in &roads {
            roads_quadtree.insert_with_box(r.id, r.get_bbox_for_road());
            if let Some(bbox) = r.get_bbox_for_icon() {
                road_icons_quadtree.insert_with_box(r.id, bbox);
            }
        }
        let mut intersections_quadtree = QuadTree::default(map_bbox);
        for i in &intersections {
            intersections_quadtree.insert_with_box(i.id, i.get_bbox());
        }
        let mut turn_icons_quadtree = QuadTree::default(map_bbox);
        for t in &turns {
            turn_icons_quadtree.insert_with_box(t.id, t.get_bbox());
        }
        let mut buildings_quadtree = QuadTree::default(map_bbox);
        for b in &buildings {
            buildings_quadtree.insert_with_box(b.id, b.get_bbox());
        }
        let mut parcels_quadtree = QuadTree::default(map_bbox);
        for p in &parcels {
            parcels_quadtree.insert_with_box(p.id, p.get_bbox());
        }

        (
            DrawMap {
                roads,
                intersections,
                turns,
                buildings,
                parcels,

                roads_quadtree,
                road_icons_quadtree,
                intersections_quadtree,
                turn_icons_quadtree,
                buildings_quadtree,
                parcels_quadtree,
            },
            bounds,
            Pt2D::new(max_screen_pt.x() / 2.0, max_screen_pt.y() / 2.0),
            max_screen_pt,
        )
    }

    // The alt to these is implementing std::ops::Index, but that's way more verbose!
    pub fn get_r(&self, id: RoadID) -> &DrawRoad {
        &self.roads[id.0]
    }

    pub fn get_i(&self, id: IntersectionID) -> &DrawIntersection {
        &self.intersections[id.0]
    }

    pub fn get_t(&self, id: TurnID) -> &DrawTurn {
        &self.turns[id.0]
    }

    pub fn get_b(&self, id: BuildingID) -> &DrawBuilding {
        &self.buildings[id.0]
    }

    pub fn get_p(&self, id: ParcelID) -> &DrawParcel {
        &self.parcels[id.0]
    }

    pub fn get_roads_onscreen(&self, screen_bbox: Rect) -> Vec<&DrawRoad> {
        let mut v = Vec::new();
        for &(id, _, _) in &self.roads_quadtree.query(screen_bbox) {
            v.push(self.get_r(*id));
        }
        v
    }

    pub fn get_road_icons_onscreen(&self, screen_bbox: Rect) -> Vec<&DrawRoad> {
        let mut v = Vec::new();
        for &(id, _, _) in &self.road_icons_quadtree.query(screen_bbox) {
            v.push(self.get_r(*id));
        }
        v
    }

    pub fn get_intersections_onscreen(&self, screen_bbox: Rect) -> Vec<&DrawIntersection> {
        let mut v = Vec::new();
        for &(id, _, _) in &self.intersections_quadtree.query(screen_bbox) {
            v.push(self.get_i(*id));
        }
        v
    }

    pub fn get_turn_icons_onscreen(&self, screen_bbox: Rect) -> Vec<&DrawTurn> {
        let mut v = Vec::new();
        for &(id, _, _) in &self.turn_icons_quadtree.query(screen_bbox) {
            v.push(self.get_t(*id));
        }
        v
    }

    pub fn get_buildings_onscreen(&self, screen_bbox: Rect) -> Vec<&DrawBuilding> {
        let mut v = Vec::new();
        for &(id, _, _) in &self.buildings_quadtree.query(screen_bbox) {
            v.push(self.get_b(*id));
        }
        v
    }

    pub fn get_parcels_onscreen(&self, screen_bbox: Rect) -> Vec<&DrawParcel> {
        let mut v = Vec::new();
        for &(id, _, _) in &self.parcels_quadtree.query(screen_bbox) {
            v.push(self.get_p(*id));
        }
        v
    }
}

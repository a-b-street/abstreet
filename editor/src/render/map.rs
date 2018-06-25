// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::QuadTree;
use aabb_quadtree::geom::{Point, Rect};
use map_model::geometry;
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
    intersections_quadtree: QuadTree<IntersectionID>,
    turn_icons_quadtree: QuadTree<TurnID>,
    buildings_quadtree: QuadTree<BuildingID>,
    parcels_quadtree: QuadTree<ParcelID>,
}

impl DrawMap {
    // Also returns the center of the map in map-space
    pub fn new(map: &Map) -> (DrawMap, Bounds, Pt2D) {
        let bounds = map.get_gps_bounds();

        let mut roads: Vec<DrawRoad> = Vec::new();
        for r in map.all_roads() {
            roads.push(DrawRoad::new(r));
        }

        let mut turn_to_road_offset: HashMap<TurnID, usize> = HashMap::new();
        for r in map.all_roads() {
            let mut turns = map.get_turns_from_road(r.id);
            // Sort the turn icons by angle.
            turns.sort_by_key(|t| {
                let src_pt = map.get_r(t.src).last_pt();
                let dst_pt = map.get_r(t.dst).first_pt();
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
            .map(|t| DrawTurn::new(map, t, turn_to_road_offset[&t.id]))
            .collect();
        let intersections: Vec<DrawIntersection> = map.all_intersections()
            .iter()
            .map(|i| DrawIntersection::new(i, &roads))
            .collect();
        let buildings: Vec<DrawBuilding> = map.all_buildings()
            .iter()
            .map(|b| DrawBuilding::new(b, map))
            .collect();
        let parcels: Vec<DrawParcel> = map.all_parcels()
            .iter()
            .map(|p| DrawParcel::new(p))
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
        for r in &roads {
            roads_quadtree.insert_with_box(r.id, r.get_bbox_for_road());
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
                intersections_quadtree,
                turn_icons_quadtree,
                buildings_quadtree,
                parcels_quadtree,
            },
            bounds,
            Pt2D::new(max_screen_pt.x() / 2.0, max_screen_pt.y() / 2.0),
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

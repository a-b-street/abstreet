// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate geo;
extern crate map_model;

use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::geometry;
use map_model::{Bounds, BuildingID, Map};
use ordered_float::NotNaN;
use std::f64;

#[derive(Debug)]
pub struct DrawBuilding {
    pub id: BuildingID,
    polygon: Vec<Vec2d>,
    // TODO this belongs in GeomBuilding, as soon as we use it for people walking to and from
    // buildings
    front_path: Option<[f64; 4]>,
}

impl DrawBuilding {
    pub fn new(bldg: &map_model::Building, bounds: &Bounds, map: &Map) -> DrawBuilding {
        let pts: Vec<Vec2d> = bldg.points
            .iter()
            .map(|pt| {
                let screen_pt = geometry::gps_to_screen_space(pt, bounds);
                [screen_pt.x(), screen_pt.y()]
            })
            .collect();
        DrawBuilding {
            id: bldg.id,
            // TODO ideally start the path on a side of the building
            front_path: find_front_path(bldg.id, center(&pts), map),
            polygon: pts,
        }
    }

    // TODO it'd be cool to draw a thick border. how to expand a polygon?
    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        if let Some(line) = self.front_path {
            let path = graphics::Line::new_round([0.0, 0.6, 0.0, 1.0], 1.0);
            path.draw(line, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }

        let poly = graphics::Polygon::new(color);
        poly.draw(&self.polygon, &g.ctx.draw_state, g.ctx.transform, g.gfx);
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.polygon)
    }

    pub fn tooltip_lines(&self, map: &Map) -> Vec<String> {
        let b = map.get_b(self.id);
        let mut lines = vec![
            format!("Building #{:?} (from OSM way {})", self.id, b.osm_way_id),
        ];
        lines.extend(b.osm_tags.iter().cloned());
        lines
    }

    pub fn get_bbox(&self) -> Rect {
        geometry::get_bbox_for_polygons(&[self.polygon.clone()])
    }
}

fn center(pts: &Vec<Vec2d>) -> Vec2d {
    let mut x = 0.0;
    let mut y = 0.0;
    for pt in pts {
        x += pt[0];
        y += pt[1];
    }
    let len = pts.len() as f64;
    [x / len, y / len]
}

fn road_to_line_string(r: map_model::RoadID, map: &Map) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = map.get_r(r)
        .lane_center_lines
        .iter()
        .flat_map(|pair| {
            vec![
                geo::Point::new(pair.0.x(), pair.0.y()),
                geo::Point::new(pair.1.x(), pair.1.y()),
            ]
        })
        .collect();
    pts.into()
}

fn find_front_path(id: BuildingID, bldg_center: Vec2d, map: &Map) -> Option<[f64; 4]> {
    use geo::prelude::{ClosestPoint, EuclideanDistance};

    if let Some(tag) = map.get_b(id)
        .osm_tags
        .iter()
        .find(|kv| kv.starts_with("addr:street="))
    {
        let (_, street_name) = tag.split_at("addr:street=".len());

        let center_pt = geo::Point::new(bldg_center[0], bldg_center[1]);

        // Find all matching sidewalks with that street name, then find the closest point on
        // that sidewalk
        let candidates: Vec<(map_model::RoadID, geo::Point<f64>)> = map.all_roads()
            .iter()
            .filter_map(|r| {
                if r.lane_type == map_model::LaneType::Sidewalk
                    && map_model::has_osm_tag(&r.osm_tags, "name", street_name)
                {
                    if let geo::Closest::SinglePoint(pt) =
                        road_to_line_string(r.id, map).closest_point(&center_pt)
                    {
                        return Some((r.id, pt));
                    }
                }
                None
            })
            .collect();

        if let Some(closest) = candidates
            .iter()
            .min_by_key(|pair| NotNaN::new(pair.1.euclidean_distance(&center_pt)).unwrap())
        {
            return Some([center_pt.x(), center_pt.y(), closest.1.x(), closest.1.y()]);
        }
    }
    None
}

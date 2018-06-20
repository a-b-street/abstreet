// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate geo;
extern crate map_model;

use aabb_quadtree::geom::Rect;
use ezgui::canvas::GfxCtx;
use geom::GeomMap;
use geom::geometry;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::{Bounds, BuildingID};
use ordered_float::NotNaN;
use render;
use std::f64;

#[derive(Debug)]
pub struct DrawBuilding {
    pub id: BuildingID,
    polygon: Vec<Vec2d>,
}

impl DrawBuilding {
    pub fn new(bldg: &map_model::Building, bounds: &Bounds) -> DrawBuilding {
        DrawBuilding {
            id: bldg.id,
            polygon: bldg.points
                .iter()
                .map(|pt| {
                    let screen_pt = geometry::gps_to_screen_space(pt, bounds);
                    [screen_pt.x(), screen_pt.y()]
                })
                .collect(),
        }
    }

    // TODO it'd be cool to draw a thick border. how to expand a polygon?
    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        poly.draw(&self.polygon, &g.ctx.draw_state, g.ctx.transform, g.gfx);
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.polygon)
    }

    pub fn tooltip_lines(&self, map: &map_model::Map) -> Vec<String> {
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

    // TODO compute these once, and draw them always
    // TODO ideally start the path on a side of the building
    pub fn draw_sidewalk_path(&self, g: &mut GfxCtx, map: &map_model::Map, geom_map: &GeomMap) {
        use geo::prelude::{ClosestPoint, EuclideanDistance};

        if let Some(tag) = map.get_b(self.id)
            .osm_tags
            .iter()
            .find(|kv| kv.starts_with("addr:street="))
        {
            let (_, street_name) = tag.split_at("addr:street=".len());

            let bldg_center = self.center();
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
                            road_to_line_string(r.id, geom_map).closest_point(&center_pt)
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
                let path = graphics::Line::new_round(render::DEBUG_COLOR, 1.0);
                path.draw(
                    [center_pt.x(), center_pt.y(), closest.1.x(), closest.1.y()],
                    &g.ctx.draw_state,
                    g.ctx.transform,
                    g.gfx,
                );
            }
        }
    }

    fn center(&self) -> Vec2d {
        let mut x = 0.0;
        let mut y = 0.0;
        for pt in &self.polygon {
            x += pt[0];
            y += pt[1];
        }
        let len = self.polygon.len() as f64;
        [x / len, y / len]
    }
}

fn road_to_line_string(r: map_model::RoadID, geom_map: &GeomMap) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = geom_map
        .get_r(r)
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

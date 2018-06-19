// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate dimensioned;
extern crate map_model;

use BIG_ARROW_THICKNESS;
use LANE_THICKNESS;
use TURN_DIST_FROM_INTERSECTION;

use dimensioned::si;
use geometry;
use graphics::math::Vec2d;
use map_model::{Bounds, Pt2D, RoadID};
use std::f64;

#[derive(Debug)]
pub struct GeomRoad {
    pub id: RoadID,
    // TODO need to settle on a proper Line type
    pub lane_center_lines: Vec<(Pt2D, Pt2D)>,
    // unshifted center points. consider computing these twice or otherwise not storing them
    pub pts: Vec<Pt2D>,
}

impl GeomRoad {
    pub fn new(road: &map_model::Road, bounds: &Bounds) -> GeomRoad {
        let mut pts: Vec<Pt2D> = road.points
            .iter()
            .map(|pt| geometry::gps_to_screen_space(pt, bounds))
            .collect();
        // Shove the lines away from the intersection so they don't overlap.
        // TODO deal with tiny roads
        let num_pts = pts.len();
        let new_first_pt =
            geometry::dist_along_line((&pts[0], &pts[1]), TURN_DIST_FROM_INTERSECTION);
        let new_last_pt = geometry::dist_along_line(
            (&pts[num_pts - 1], &pts[num_pts - 2]),
            TURN_DIST_FROM_INTERSECTION,
        );
        pts[0] = Pt2D::from(new_first_pt);
        pts[num_pts - 1] = Pt2D::from(new_last_pt);

        // TODO handle offset
        let lane_center_shift = if road.one_way_road {
            0.0
        } else if road.use_yellow_center_lines {
            // TODO I think this is unfair to one side, right? If we hover over the yellow line, it
            // shouldn't match either lane. Needs to be its own thing, or adjust the bbox.
            (LANE_THICKNESS / 2.0) + (BIG_ARROW_THICKNESS / 2.0)
        } else {
            (LANE_THICKNESS / 2.0)
        };
        // TODO when drawing cars along these lines, do we have the line overlap problem?
        let lane_center_lines: Vec<(Pt2D, Pt2D)> = pts.windows(2)
            .map(|pair| {
                geometry::shift_line_perpendicularly_in_driving_direction(
                    lane_center_shift,
                    &pair[0],
                    &pair[1],
                )
            })
            .collect();

        GeomRoad {
            lane_center_lines,
            pts,
            id: road.id,
        }
    }

    pub fn first_pt(&self) -> Vec2d {
        let pt = &self.lane_center_lines[0].0;
        [pt.x(), pt.y()]
    }

    pub fn last_pt(&self) -> Vec2d {
        let pt = &self.lane_center_lines[self.lane_center_lines.len() - 1].1;
        [pt.x(), pt.y()]
    }

    pub fn last_line(&self) -> (Pt2D, Pt2D) {
        self.lane_center_lines[self.lane_center_lines.len() - 1]
    }

    pub fn dist_along(&self, dist_along: si::Meter<f64>) -> (Pt2D, geometry::angles::Radian<f64>) {
        // TODO valid to do euclidean distance on screen-space points that're formed from
        // Haversine?
        let mut dist_left = dist_along;
        for (idx, l) in self.lane_center_lines.iter().enumerate() {
            let length = geometry::euclid_dist((&l.0, &l.1));
            let epsilon = if idx == self.lane_center_lines.len() - 1 {
                geometry::EPSILON_METERS
            } else {
                0.0 * si::M
            };
            if dist_left <= length + epsilon {
                let vec = geometry::safe_dist_along_line((&l.0, &l.1), dist_left);
                return (Pt2D::new(vec[0], vec[1]), geometry::angle(&l.0, &l.1));
            }
            dist_left -= length;
        }
        panic!(
            "{} is longer than road {:?}'s {}. dist_left is {}",
            dist_along,
            self.id,
            self.length(),
            dist_left
        );
    }

    pub fn length(&self) -> si::Meter<f64> {
        self.lane_center_lines
            .iter()
            .fold(0.0 * si::M, |so_far, l| {
                so_far + geometry::euclid_dist((&l.0, &l.1))
            })
    }
}

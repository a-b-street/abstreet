// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Bounds;
use Pt2D;
use dimensioned::si;
use geometry;
use graphics::math::Vec2d;
use std::f64;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct RoadID(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum LaneType {
    Driving,
    Parking,
    Sidewalk,
}

#[derive(Debug)]
pub struct Road {
    pub id: RoadID,
    pub osm_tags: Vec<String>,
    pub osm_way_id: i64,
    pub lane_type: LaneType,

    // Ideally all of these would just become translated center points immediately, but this is
    // hard due to the polyline problem.

    // All roads are two-way (since even one-way streets have sidewalks on both sides). Offset 0 is
    // the centermost lane on each side, then it counts up.
    pub offset: u8,
    // The orientation is implied by the order of these points
    // These're GPS
    pub points: Vec<Pt2D>,
    // Should this lane own the drawing of the yellow center lines? For two-way roads, this is
    // arbitrarily grouped with one of the lanes. Ideally it would be owned by something else.
    pub use_yellow_center_lines: bool,
    // Need to remember this just for detecting U-turns here.
    pub(crate) other_side: Option<RoadID>,

    /// GeomRoad stuff
    // TODO need to settle on a proper Line type
    pub lane_center_lines: Vec<(Pt2D, Pt2D)>,
    // unshifted center points. consider computing these twice or otherwise not storing them
    // These're screen
    pub pts: Vec<Pt2D>,
}

impl PartialEq for Road {
    fn eq(&self, other: &Road) -> bool {
        self.id == other.id
    }
}

impl Road {
    pub fn first_pt(&self) -> Vec2d {
        let pt = &self.lane_center_lines[0].0;
        [pt.x(), pt.y()]
    }

    pub fn last_pt(&self) -> Vec2d {
        let pt = &self.lane_center_lines.last().unwrap().1;
        [pt.x(), pt.y()]
    }

    pub fn last_line(&self) -> (Pt2D, Pt2D) {
        *self.lane_center_lines.last().unwrap()
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

// Returns lane_center_lines, pts
pub(crate) fn calculate_geometry(
    gps_points: &Vec<Pt2D>,
    offset: u8,
    use_yellow_center_lines: bool,
    bounds: &Bounds,
) -> (Vec<(Pt2D, Pt2D)>, Vec<Pt2D>) {
    let mut pts: Vec<Pt2D> = gps_points
        .iter()
        .map(|pt| geometry::gps_to_screen_space(pt, bounds))
        .collect();
    // Shove the lines away from the intersection so they don't overlap.
    // TODO deal with tiny roads
    let num_pts = pts.len();
    let new_first_pt =
        geometry::dist_along_line((&pts[0], &pts[1]), geometry::TURN_DIST_FROM_INTERSECTION);
    let new_last_pt = geometry::dist_along_line(
        (&pts[num_pts - 1], &pts[num_pts - 2]),
        geometry::TURN_DIST_FROM_INTERSECTION,
    );
    pts[0] = Pt2D::from(new_first_pt);
    pts[num_pts - 1] = Pt2D::from(new_last_pt);

    let lane_center_shift = if use_yellow_center_lines {
        // TODO I think this is unfair to one side, right? If we hover over the yellow line, it
        // shouldn't match either lane. Needs to be its own thing, or adjust the bbox.
        (geometry::LANE_THICKNESS / 2.0) + (geometry::BIG_ARROW_THICKNESS / 2.0)
    } else {
        geometry::LANE_THICKNESS * ((offset as f64) + 0.5)
    };
    // TODO when drawing cars along these lines, do we have the line overlap problem? yes.
    let lane_center_lines: Vec<(Pt2D, Pt2D)> = pts.windows(2)
        .map(|pair| {
            geometry::shift_line_perpendicularly_in_driving_direction(
                lane_center_shift,
                &pair[0],
                &pair[1],
            )
        })
        .collect();

    (lane_center_lines, pts)
}

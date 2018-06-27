// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use IntersectionID;
use Pt2D;
use dimensioned::si;
use geometry;
use graphics::math::Vec2d;
use std::collections::HashMap;
use std::f64;
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct RoadID(pub usize);

impl fmt::Display for RoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RoadID({0})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum LaneType {
    Driving,
    Parking,
    Sidewalk,
}

#[derive(Debug)]
pub struct Road {
    pub id: RoadID,
    pub osm_tags: HashMap<String, String>,
    pub osm_way_id: i64,
    pub lane_type: LaneType,

    pub(crate) src_i: IntersectionID,
    pub(crate) dst_i: IntersectionID,

    // Ideally all of these would just become translated center points immediately, but this is
    // hard due to the polyline problem.

    // All roads are two-way (since even one-way streets have sidewalks on both sides). Offset 0 is
    // the centermost lane on each side, then it counts up.
    pub offset: u8,
    // Should this lane own the drawing of the yellow center lines? For two-way roads, this is
    // arbitrarily grouped with one of the lanes. Ideally it would be owned by something else.
    pub use_yellow_center_lines: bool,
    // Need to remember this just for detecting U-turns here.
    pub(crate) other_side: Option<RoadID>,

    /// GeomRoad stuff
    pub lane_center_pts: Vec<Pt2D>,

    // Unshifted center points. consider computing these twice or otherwise not storing them
    // These're screen-space. Order implies road orientation.
    pub unshifted_pts: Vec<Pt2D>,
}

impl PartialEq for Road {
    fn eq(&self, other: &Road) -> bool {
        self.id == other.id
    }
}

impl Road {
    pub fn first_pt(&self) -> Vec2d {
        self.lane_center_pts[0].to_vec()
    }
    pub fn last_pt(&self) -> Vec2d {
        self.lane_center_pts.last().unwrap().to_vec()
    }
    pub fn first_line(&self) -> (Pt2D, Pt2D) {
        (self.lane_center_pts[0], self.lane_center_pts[1])
    }
    pub fn last_line(&self) -> (Pt2D, Pt2D) {
        (
            self.lane_center_pts[self.lane_center_pts.len() - 2],
            self.lane_center_pts[self.lane_center_pts.len() - 1],
        )
    }

    pub fn dist_along(&self, dist_along: si::Meter<f64>) -> (Pt2D, geometry::angles::Radian<f64>) {
        geometry::dist_along(&self.lane_center_pts, dist_along)
    }

    pub fn length(&self) -> si::Meter<f64> {
        geometry::polyline_len(&self.lane_center_pts)
    }

    pub fn dump_debug(&self) {
        println!("\nlet debug_r{}_pts = vec![", self.id.0);
        for pt in &self.lane_center_pts {
            println!("  Pt2D::new({}, {}),", pt.x(), pt.y());
        }
        println!("];");
    }
}

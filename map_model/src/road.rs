// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use IntersectionID;
use dimensioned::si;
use geom::{Angle, Line, PolyLine, Pt2D};
use std::collections::HashMap;
use std::f64;
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Serialize, Deserialize)]
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

    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    // Ideally all of these would just become translated center points immediately, but this is
    // hard due to the polyline problem.

    // All roads are two-way (since even one-way streets have sidewalks on both sides). Offset 0 is
    // the centermost lane on each side, then it counts up.
    pub offset: u8,
    // Should this lane own the drawing of the yellow center lines? For two-way roads, this is
    // arbitrarily grouped with one of the lanes. Ideally it would be owned by something else.
    pub use_yellow_center_lines: bool,
    // Need to remember this just for detecting U-turns here. Also for finding sidewalks to connect
    // with a crosswalk.
    pub other_side: Option<RoadID>,

    /// GeomRoad stuff
    pub lane_center_pts: PolyLine,

    // Remember that lane_center_pts and derived geometry is probably broken. Might be better to
    // use this breakage to infer that a road doesn't have so many lanes.
    pub probably_broken: bool,

    // Unshifted center points. consider computing these twice or otherwise not storing them
    // Order implies road orientation.
    pub unshifted_pts: PolyLine,
}

impl PartialEq for Road {
    fn eq(&self, other: &Road) -> bool {
        self.id == other.id
    }
}

impl Road {
    // TODO most of these are wrappers; stop doing this?
    pub fn first_pt(&self) -> Pt2D {
        self.lane_center_pts.first_pt()
    }
    pub fn last_pt(&self) -> Pt2D {
        self.lane_center_pts.last_pt()
    }
    pub fn first_line(&self) -> Line {
        self.lane_center_pts.first_line()
    }
    pub fn last_line(&self) -> Line {
        self.lane_center_pts.last_line()
    }

    pub fn dist_along(&self, dist_along: si::Meter<f64>) -> (Pt2D, Angle) {
        self.lane_center_pts.dist_along(dist_along)
    }

    pub fn length(&self) -> si::Meter<f64> {
        self.lane_center_pts.length()
    }

    pub fn dump_debug(&self) {
        println!("\nlet debug_r{}_pts = vec![", self.id.0);
        // TODO nicer display for PolyLine?
        for pt in self.lane_center_pts.points().iter() {
            println!("  Pt2D::new({}, {}),", pt.x(), pt.y());
        }
        println!("];");
    }
}

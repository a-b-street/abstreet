// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use geom::{Angle, Line, PolyLine, Pt2D};
use std;
use std::collections::HashMap;
use std::f64;
use std::fmt;
use IntersectionID;

const PARKING_SPOT_LENGTH: si::Meter<f64> = si::Meter {
    // TODO look up a real value
    value_unsafe: 10.0,
    _marker: std::marker::PhantomData,
};

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
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
    // TODO alright, we might need a Road-vs-Lanes distinction
    pub siblings: Vec<RoadID>,

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
        println!(
            "\nlet lane_center_r{}_pts = {}",
            self.id.0, self.lane_center_pts
        );
        println!(
            "\nlet unshifted_r{}_pts = {}",
            self.id.0, self.unshifted_pts
        );
    }

    // TODO different types for each lane type might be reasonable

    pub fn number_parking_spots(&self) -> usize {
        assert_eq!(self.lane_type, LaneType::Parking);
        // No spots next to intersections
        let spots = (self.length() / PARKING_SPOT_LENGTH).floor() - 2.0;
        if spots >= 1.0 {
            spots as usize
        } else {
            0
        }
    }

    // Returns the front of the spot. Can handle [0, number_parking_spots()] inclusive -- the last
    // value is for rendering the last marking.
    pub fn parking_spot_position(&self, spot_idx: usize) -> (Pt2D, Angle) {
        assert_eq!(self.lane_type, LaneType::Parking);
        // +1 to start away from the intersection
        self.dist_along(PARKING_SPOT_LENGTH * (1.0 + spot_idx as f64))
    }
}

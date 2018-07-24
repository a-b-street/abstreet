// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use geom::{Angle, Line, PolyLine, Pt2D};
use std;
use std::f64;
use std::fmt;
use {IntersectionID, RoadID};

pub const PARKING_SPOT_LENGTH: si::Meter<f64> = si::Meter {
    // TODO look up a real value
    value_unsafe: 10.0,
    _marker: std::marker::PhantomData,
};

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LaneID(pub usize);

impl fmt::Display for LaneID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LaneID({0})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum LaneType {
    Driving,
    Parking,
    Sidewalk,
    Biking,
}

#[derive(Debug)]
pub struct Lane {
    pub id: LaneID,
    pub parent: RoadID,
    pub lane_type: LaneType,
    pub lane_center_pts: PolyLine,

    // Remember that lane_center_pts and derived geometry is probably broken. Might be better to
    // use this breakage to infer that a road doesn't have so many lanes.
    pub probably_broken: bool,

    // TODO i think everything else should be moved to road, honestly.
    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    // All roads are two-way (since even one-way streets have sidewalks on both sides). Offset 0 is
    // the centermost lane on each side, then it counts up.
    pub offset: u8,
    // Should this lane own the drawing of the yellow center lines? For two-way roads, this is
    // arbitrarily grouped with one of the lanes. Ideally it would be owned by something else.
    pub use_yellow_center_lines: bool,

    // Need to remember this just for detecting U-turns here. Also for finding sidewalks to connect
    // with a crosswalk.
    pub other_side: Option<LaneID>,
    // TODO alright, we might need a Road-vs-Lanes distinction
    pub siblings: Vec<LaneID>,
}

impl PartialEq for Lane {
    fn eq(&self, other: &Lane) -> bool {
        self.id == other.id
    }
}

impl Lane {
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

    pub fn endpoint(&self, i: IntersectionID) -> Pt2D {
        if i == self.src_i {
            self.first_pt()
        } else if i == self.dst_i {
            self.last_pt()
        } else {
            panic!("{} isn't an endpoint of {}", i, self.id);
        }
    }

    // pt2 will be endpoint
    pub fn end_line(&self, i: IntersectionID) -> Line {
        if i == self.src_i {
            self.first_line().reverse()
        } else if i == self.dst_i {
            self.last_line()
        } else {
            panic!("{} isn't an endpoint of {}", i, self.id);
        }
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
}

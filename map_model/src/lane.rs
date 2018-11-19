// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use dimensioned::si;
use geom::{Angle, Line, PolyLine, Pt2D};
use std;
use std::f64;
use std::fmt;
use {BuildingID, BusStopID, IntersectionID, RoadID};

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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LaneType {
    Driving,
    Parking,
    Sidewalk,
    Biking,
    Bus,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Lane {
    pub id: LaneID,
    pub parent: RoadID,
    pub lane_type: LaneType,
    pub lane_center_pts: PolyLine,

    // Remember that lane_center_pts and derived geometry is probably broken. Might be better to
    // use this breakage to infer that a road doesn't have so many lanes.
    pub probably_broken: bool,

    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    pub building_paths: Vec<BuildingID>,
    pub bus_stops: Vec<BusStopID>,
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

    pub fn safe_dist_along(&self, dist_along: si::Meter<f64>) -> Option<(Pt2D, Angle)> {
        self.lane_center_pts.safe_dist_along(dist_along)
    }

    pub fn dist_along_of_point(&self, pt: Pt2D) -> Option<si::Meter<f64>> {
        self.lane_center_pts.dist_along_of_point(pt)
    }

    pub fn length(&self) -> si::Meter<f64> {
        self.lane_center_pts.length()
    }

    pub fn dump_debug(&self) {
        println!(
            "\nlet lane_center_l{}_pts = {}",
            self.id.0, self.lane_center_pts
        );
        println!("{}", abstutil::to_json(self));
    }

    pub fn intersections(&self) -> Vec<IntersectionID> {
        // TODO I think we're assuming there are no loop lanes
        vec![self.src_i, self.dst_i]
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

    pub fn is_driving(&self) -> bool {
        self.lane_type == LaneType::Driving
    }

    pub fn is_biking(&self) -> bool {
        self.lane_type == LaneType::Biking
    }

    pub fn is_sidewalk(&self) -> bool {
        self.lane_type == LaneType::Sidewalk
    }

    pub fn is_parking(&self) -> bool {
        self.lane_type == LaneType::Parking
    }
}

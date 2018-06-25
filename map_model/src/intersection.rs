// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Pt2D;
use RoadID;
use TurnID;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct IntersectionID(pub usize);

#[derive(Debug)]
pub struct Intersection {
    pub id: IntersectionID,
    pub point: Pt2D,
    pub turns: Vec<TurnID>,
    pub elevation_meters: f64,
    pub has_traffic_signal: bool,

    pub incoming_roads: Vec<RoadID>,
    pub outgoing_roads: Vec<RoadID>,
}

impl PartialEq for Intersection {
    fn eq(&self, other: &Intersection) -> bool {
        self.id == other.id
    }
}

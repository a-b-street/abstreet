// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use IntersectionID;
use RoadID;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct TurnID(pub usize);

#[derive(Debug)]
pub struct Turn {
    pub id: TurnID,
    pub parent: IntersectionID,
    pub src: RoadID,
    pub dst: RoadID,
}

impl PartialEq for Turn {
    fn eq(&self, other: &Turn) -> bool {
        self.id == other.id
    }
}

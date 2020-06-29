use crate::{IntersectionID, RoadID};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ZoneID(pub usize);

impl fmt::Display for ZoneID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Zone #{}", self.0)
    }
}

// A contiguous set of roads with access restrictions
#[derive(Serialize, Deserialize, Debug)]
pub struct Zone {
    pub id: ZoneID,
    pub members: BTreeSet<RoadID>,
    pub borders: BTreeSet<IntersectionID>,
}

use abstutil;
use geom::{PolyLine, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParcelID(pub usize);

impl fmt::Display for ParcelID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParcelID({0})", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Parcel {
    pub id: ParcelID,
    pub points: Vec<Pt2D>,
    // All parcels of the same block have the same number.
    pub block: usize,
}

impl Parcel {
    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
        println!("{}", PolyLine::new(self.points.clone()));
    }
}

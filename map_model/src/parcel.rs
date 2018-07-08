// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use geom::Pt2D;
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParcelID(pub usize);

impl fmt::Display for ParcelID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParcelID({0})", self.0)
    }
}

#[derive(Debug)]
pub struct Parcel {
    pub id: ParcelID,
    pub points: Vec<Pt2D>,
}

impl PartialEq for Parcel {
    fn eq(&self, other: &Parcel) -> bool {
        self.id == other.id
    }
}

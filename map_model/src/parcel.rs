// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use geom::Pt2D;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ParcelID(pub usize);

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

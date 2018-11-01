use geom::Pt2D;
use std::collections::BTreeMap;
use {Tick, TripID};

#[derive(Serialize, Deserialize, PartialEq)]
pub struct SimStats {
    pub time: Tick,
    pub canonical_pt_per_trip: BTreeMap<TripID, Pt2D>,
}

impl SimStats {
    pub(crate) fn new(time: Tick) -> SimStats {
        SimStats {
            time,
            canonical_pt_per_trip: BTreeMap::new(),
        }
    }
}

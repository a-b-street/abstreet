use crate::{LaneID, LaneType, Map, Position};
use abstutil::{deserialize_usize, serialize_usize};
use geom::{Distance, PolyLine, Polygon, Pt2D};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BuildingID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for BuildingID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Building #{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Building {
    pub id: BuildingID,
    pub polygon: Polygon,
    pub address: String,
    pub name: Option<String>,
    pub osm_way_id: i64,
    // Where a text label should be centered to have the best chances of being contained within the
    // polygon.
    pub label_center: Pt2D,
    // TODO Might fold these into BuildingType::Commercial
    // (Name, amenity)
    pub amenities: BTreeSet<(String, String)>,
    pub bldg_type: BuildingType,
    pub parking: OffstreetParking,

    // The building's connection for pedestrians is immutable. For cars and bikes, it can change
    // based on map edits, so don't cache it.
    pub sidewalk_pos: Position,
    // Goes from building to sidewalk
    pub driveway_geom: PolyLine,
}

// Represent None as Private(0).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum OffstreetParking {
    // (Name, spots)
    PublicGarage(String, usize),
    Private(usize),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BuildingType {
    // An estimated number of residents
    Residential(usize),
    ResidentialCommercial(usize),
    Commercial,
    Empty,
}

impl BuildingType {
    pub fn has_residents(&self) -> bool {
        match self {
            BuildingType::Residential(_) | BuildingType::ResidentialCommercial(_) => true,
            BuildingType::Commercial | BuildingType::Empty => false,
        }
    }
}

impl Building {
    pub fn sidewalk(&self) -> LaneID {
        self.sidewalk_pos.lane()
    }

    pub fn house_number(&self) -> Option<String> {
        let num = self.address.split(" ").next().unwrap();
        if num != "???" {
            Some(num.to_string())
        } else {
            None
        }
    }

    // The polyline goes from the building to the driving position
    pub fn driving_connection(&self, map: &Map) -> Option<(Position, PolyLine)> {
        // Is there even a driving lane on the same side as our sidewalk?
        // TODO Handle offside
        let lane = map
            .get_parent(self.sidewalk())
            .find_closest_lane(self.sidewalk(), vec![LaneType::Driving])
            .ok()?;
        let pos = self.sidewalk_pos.equiv_pos(lane, Distance::ZERO, map);

        // TODO Do we need to insist on this buffer, now that we can make cars gradually appear?
        let buffer = Distance::meters(7.0);
        if pos.dist_along() <= buffer || map.get_l(lane).length() - pos.dist_along() <= buffer {
            return None;
        }
        Some((pos, self.driveway_geom.clone().must_push(pos.pt(map))))
    }

    pub fn num_parking_spots(&self) -> usize {
        match self.parking {
            OffstreetParking::PublicGarage(_, n) => n,
            OffstreetParking::Private(n) => n,
        }
    }
}

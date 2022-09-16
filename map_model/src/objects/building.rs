use std::collections::{HashSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize, Tags};
use geom::{Distance, PolyLine, Polygon, Pt2D};

use crate::{osm, Amenity, AmenityType, LaneID, Map, NamePerLanguage, PathConstraints, Position};

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

/// A building has connections to the road and sidewalk, may contain commercial amenities, and have
/// off-street parking.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Building {
    pub id: BuildingID,
    pub polygon: Polygon,
    pub levels: f64,
    pub address: String,
    pub name: Option<NamePerLanguage>,
    pub orig_id: osm::OsmID,
    /// Where a text label should be centered to have the best chances of being contained within
    /// the polygon.
    pub label_center: Pt2D,
    pub amenities: Vec<Amenity>,
    pub bldg_type: BuildingType,
    pub parking: OffstreetParking,
    /// Depending on options while importing, these might be empty, to save file space.
    pub osm_tags: Tags,

    /// The building's connection for any agent can change based on map edits. Just store the one
    /// for pedestrians and lazily calculate the others.
    pub sidewalk_pos: Position,
    /// Goes from building to sidewalk
    pub driveway_geom: PolyLine,
}

/// Represent no parking as Private(0, false).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum OffstreetParking {
    /// (Name, spots)
    PublicGarage(String, usize),
    /// (Spots, explicitly tagged as a garage)
    Private(usize, bool),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BuildingType {
    Residential {
        num_residents: usize,
        num_housing_units: usize,
    },
    /// An estimated number of residents, workers
    ResidentialCommercial(usize, usize),
    /// An estimated number of workers
    Commercial(usize),
    Empty,
}

impl BuildingType {
    pub fn has_residents(&self) -> bool {
        match self {
            BuildingType::Residential { .. } | BuildingType::ResidentialCommercial(_, _) => true,
            BuildingType::Commercial(_) | BuildingType::Empty => false,
        }
    }
}

impl Building {
    pub fn sidewalk(&self) -> LaneID {
        self.sidewalk_pos.lane()
    }

    /// The polyline goes from the building to the driving position
    // TODO Make this handle parking_blackhole
    pub fn driving_connection(&self, map: &Map) -> Option<(Position, PolyLine)> {
        let lane = map
            .get_parent(self.sidewalk())
            .find_closest_lane(self.sidewalk(), |l| PathConstraints::Car.can_use(l, map))?;
        // TODO Do we need to insist on this buffer, now that we can make cars gradually appear?
        let pos = self
            .sidewalk_pos
            .equiv_pos(lane, map)
            .buffer_dist(Distance::meters(7.0), map)?;
        Some((pos, self.driveway_geom.clone().optionally_push(pos.pt(map))))
    }

    /// Returns (biking position, sidewalk position). Could fail if the biking graph is
    /// disconnected.
    pub fn biking_connection(&self, map: &Map) -> Option<(Position, Position)> {
        // Easy case: the building is directly next to a usable lane
        if let Some(pair) = sidewalk_to_bike(self.sidewalk_pos, map) {
            return Some(pair);
        }

        // Floodfill the sidewalk graph until we find a sidewalk<->bike connection.
        let mut queue: VecDeque<LaneID> = VecDeque::new();
        let mut visited: HashSet<LaneID> = HashSet::new();
        queue.push_back(self.sidewalk());

        loop {
            if queue.is_empty() {
                return None;
            }
            let l = queue.pop_front().unwrap();
            if visited.contains(&l) {
                continue;
            }
            visited.insert(l);
            // TODO Could search by sidewalk endpoint
            if let Some(pair) = sidewalk_to_bike(Position::new(l, map.get_l(l).length() / 2.0), map)
            {
                return Some(pair);
            }
            for (_, next) in map.get_next_turns_and_lanes(l) {
                if next.is_walkable() && !visited.contains(&next.id) {
                    queue.push_back(next.id);
                }
            }
        }
    }

    pub fn num_parking_spots(&self) -> usize {
        match self.parking {
            OffstreetParking::PublicGarage(_, n) => n,
            OffstreetParking::Private(n, _) => n,
        }
    }

    /// Does this building contain any amenity matching the category?
    pub fn has_amenity(&self, category: AmenityType) -> bool {
        for amenity in &self.amenities {
            if AmenityType::categorize(&amenity.amenity_type) == Some(category) {
                return true;
            }
        }
        false
    }
}

fn sidewalk_to_bike(sidewalk_pos: Position, map: &Map) -> Option<(Position, Position)> {
    let lane = map
        .get_parent(sidewalk_pos.lane())
        .find_closest_lane(sidewalk_pos.lane(), |l| {
            !l.biking_blackhole && PathConstraints::Bike.can_use(l, map)
        })?;
    // No buffer needed
    Some((sidewalk_pos.equiv_pos(lane, map), sidewalk_pos))
}

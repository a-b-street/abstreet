use crate::{osm, LaneID, Map, PathConstraints, Position};
use abstutil::{
    deserialize_btreemap, deserialize_usize, serialize_btreemap, serialize_usize, Tags,
};
use geom::{Distance, PolyLine, Polygon, Pt2D};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
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
    pub levels: f64,
    pub address: String,
    pub name: Option<NamePerLanguage>,
    pub orig_id: osm::OsmID,
    // Where a text label should be centered to have the best chances of being contained within the
    // polygon.
    pub label_center: Pt2D,
    // TODO Might fold these into BuildingType::Commercial
    // (Name, amenity)
    pub amenities: BTreeSet<(NamePerLanguage, String)>,
    pub bldg_type: BuildingType,
    pub parking: OffstreetParking,

    // The building's connection for pedestrians is immutable. For cars and bikes, it can change
    // based on map edits, so don't cache it.
    pub sidewalk_pos: Position,
    // Goes from building to sidewalk
    pub driveway_geom: PolyLine,
}

// Represent None as Private(0, false).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum OffstreetParking {
    // (Name, spots)
    PublicGarage(String, usize),
    // (Spots, explicitly tagged as a garage)
    Private(usize, bool),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BuildingType {
    // An estimated number of residents
    Residential(usize),
    // An estimated number of residents, workers
    ResidentialCommercial(usize, usize),
    // An estimated number of workers
    Commercial(usize),
    Empty,
}

impl BuildingType {
    pub fn has_residents(&self) -> bool {
        match self {
            BuildingType::Residential(_) | BuildingType::ResidentialCommercial(_, _) => true,
            BuildingType::Commercial(_) | BuildingType::Empty => false,
        }
    }
}

// None corresponds to the native name
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct NamePerLanguage(
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub(crate) BTreeMap<Option<String>, String>,
);

impl NamePerLanguage {
    pub fn get(&self, lang: Option<&String>) -> &String {
        // TODO Can we avoid this clone?
        let lang = lang.cloned();
        if let Some(name) = self.0.get(&lang) {
            return name;
        }
        &self.0[&None]
    }

    pub fn new(tags: &Tags) -> Option<NamePerLanguage> {
        let native_name = tags.get(osm::NAME)?;
        let mut map = BTreeMap::new();
        map.insert(None, native_name.to_string());
        for (k, v) in tags.inner() {
            if let Some(lang) = k.strip_prefix("name:") {
                map.insert(Some(lang.to_string()), v.to_string());
            }
        }
        Some(NamePerLanguage(map))
    }

    pub fn unnamed() -> NamePerLanguage {
        let mut map = BTreeMap::new();
        map.insert(None, "unnamed".to_string());
        NamePerLanguage(map)
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
    // TODO Make this handle parking_blackhole
    pub fn driving_connection(&self, map: &Map) -> Option<(Position, PolyLine)> {
        let lane = map.get_parent(self.sidewalk()).find_closest_lane(
            self.sidewalk(),
            |l| PathConstraints::Car.can_use(l, map),
            map,
        )?;
        // TODO Do we need to insist on this buffer, now that we can make cars gradually appear?
        let pos = self
            .sidewalk_pos
            .equiv_pos(lane, map)
            .buffer_dist(Distance::meters(7.0), map)?;
        Some((pos, self.driveway_geom.clone().must_push(pos.pt(map))))
    }

    // Returns (biking position, sidewalk position). Could fail if the biking graph is
    // disconnected.
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
            for t in map.get_turns_from_lane(l) {
                if !visited.contains(&t.id.dst) {
                    queue.push_back(t.id.dst);
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
}

fn sidewalk_to_bike(sidewalk_pos: Position, map: &Map) -> Option<(Position, Position)> {
    let lane = map.get_parent(sidewalk_pos.lane()).find_closest_lane(
        sidewalk_pos.lane(),
        |l| !l.biking_blackhole && PathConstraints::Bike.can_use(l, map),
        map,
    )?;
    // No buffer needed
    Some((sidewalk_pos.equiv_pos(lane, map), sidewalk_pos))
}

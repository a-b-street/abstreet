use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{
    deserialize_btreemap, deserialize_usize, serialize_btreemap, serialize_usize, Tags,
};
use geom::{Distance, PolyLine, Polygon, Pt2D};

use crate::{osm, LaneID, Map, PathConstraints, Position};

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
#[derive(Serialize, Deserialize, Debug)]
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

    /// The building's connection for pedestrians is immutable. For cars and bikes, it can change
    /// based on map edits, so don't cache it.
    pub sidewalk_pos: Position,
    /// Goes from building to sidewalk
    pub driveway_geom: PolyLine,
}

/// A business located inside a building.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Amenity {
    pub names: NamePerLanguage,
    /// This is the specific amenity listed in OSM, not the more general `AmenityType` category.
    pub amenity_type: String,
    /// Depending on options while importing, these might be empty, to save file space.
    pub osm_tags: Tags,
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

/// None corresponds to the native name
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

    /// The polyline goes from the building to the driving position
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
    let lane = map.get_parent(sidewalk_pos.lane()).find_closest_lane(
        sidewalk_pos.lane(),
        |l| !l.biking_blackhole && PathConstraints::Bike.can_use(l, map),
        map,
    )?;
    // No buffer needed
    Some((sidewalk_pos.equiv_pos(lane, map), sidewalk_pos))
}

/// Businesses are categorized into one of these types.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AmenityType {
    Groceries,
    Food,
    Bar,
    Medical,
    Religious,
    Education,
    Financial,
    PostOffice,
    Culture,
    Childcare,
    Shopping,
}

impl AmenityType {
    fn types(self) -> Vec<&'static str> {
        // TODO: create categories for:
        // hairdresser beauty chemist
        // car_repair
        // laundry

        match self {
            AmenityType::Groceries => vec!["convenience", "supermarket"],
            // TODO Sort
            AmenityType::Food => vec![
                "restaurant",
                "cafe",
                "fast_food",
                "food_court",
                "ice_cream",
                "pastry",
                "deli",
                "greengrocer",
                "bakery",
                "butcher",
                "confectionery",
                "beverages",
                "alcohol",
            ],
            AmenityType::Bar => vec!["bar", "lounge", "pub", "nightclub"],
            AmenityType::Medical => vec![
                "chiropractor",
                "clinic",
                "dentist",
                "hospital",
                "pharmacy",
                "optician",
            ],
            AmenityType::Religious => vec!["place_of_worship"],
            AmenityType::Education => vec!["college", "school", "university"],
            AmenityType::Financial => vec!["bank"],
            AmenityType::PostOffice => vec!["post_office"],
            AmenityType::Culture => vec![
                "arts_centre",
                "art_gallery",
                "cinema",
                "library",
                "museum",
                "theatre",
            ],
            AmenityType::Childcare => vec!["childcare", "kindergarten"],
            AmenityType::Shopping => vec![
                "second_hand",
                "clothes",
                "furniture",
                "shoes",
                "department_store",
                "car",
                "kiosk",
                "hardware",
                "mobile_phone",
                "florist",
                "electronics",
                "car_parts",
                "doityourself",
                "jewelry",
                "variety_store",
                "gift",
                "bicycle",
                "books",
                "sports",
                "travel_agency",
                "stationery",
                "pet",
                "computer",
                "tyres",
                "newsagent",
            ],
        }
    }

    /// All types of amenities, in an arbitrary order.
    pub fn all() -> Vec<AmenityType> {
        vec![
            AmenityType::Groceries,
            AmenityType::Food,
            AmenityType::Bar,
            AmenityType::Medical,
            AmenityType::Religious,
            AmenityType::Education,
            AmenityType::Financial,
            AmenityType::PostOffice,
            AmenityType::Culture,
            AmenityType::Childcare,
            AmenityType::Shopping,
        ]
    }

    /// Categorize an OSM amenity tag.
    pub fn categorize(a: &str) -> Option<AmenityType> {
        for at in AmenityType::all() {
            if at.types().contains(&a) {
                return Some(at);
            }
        }
        None
    }

    pub fn parse(x: &str) -> Option<AmenityType> {
        match x {
            "groceries" => Some(AmenityType::Groceries),
            "food" => Some(AmenityType::Food),
            "bar" => Some(AmenityType::Bar),
            "medical" => Some(AmenityType::Medical),
            "religious" => Some(AmenityType::Religious),
            "education" => Some(AmenityType::Education),
            "financial" => Some(AmenityType::Financial),
            "post office" => Some(AmenityType::PostOffice),
            "culture" => Some(AmenityType::Culture),
            "childcare" => Some(AmenityType::Childcare),
            "shopping" => Some(AmenityType::Shopping),
            _ => None,
        }
    }
}

impl fmt::Display for AmenityType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AmenityType::Groceries => "groceries",
                AmenityType::Food => "food",
                AmenityType::Bar => "bar",
                AmenityType::Medical => "medical",
                AmenityType::Religious => "religious",
                AmenityType::Education => "education",
                AmenityType::Financial => "financial",
                AmenityType::PostOffice => "post office",
                AmenityType::Culture => "culture",
                AmenityType::Childcare => "childcare",
                AmenityType::Shopping => "shopping",
            }
        )
    }
}

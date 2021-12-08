use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

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
        let num = self.address.split(' ').next().unwrap();
        if num != "???" {
            Some(num.to_string())
        } else {
            None
        }
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
                if !visited.contains(&next.id) {
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

/// Businesses are categorized into one of these types.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumString, Display, EnumIter)]
pub enum AmenityType {
    Bank,
    Bar,
    Beauty,
    Bike,
    Cafe,
    CarRepair,
    CarShare,
    Childcare,
    ConvenienceStore,
    Culture,
    Exercise,
    FastFood,
    Food,
    GreenSpace,
    Hotel,
    Laundry,
    Library,
    Medical,
    Pet,
    Playground,
    Pool,
    PostOffice,
    Religious,
    School,
    Shopping,
    Supermarket,
    Tourism,
    University,
}

impl AmenityType {
    fn types(self) -> Vec<&'static str> {
        match self {
            AmenityType::Bank => vec!["bank"],
            AmenityType::Bar => vec!["bar", "pub", "nightclub", "biergarten"],
            AmenityType::Beauty => vec!["hairdresser", "beauty", "chemist", "cosmetics"],
            AmenityType::Bike => vec!["bicycle"],
            AmenityType::Cafe => vec!["cafe", "pastry", "coffee", "tea", "bakery"],
            AmenityType::CarRepair => vec!["car_repair"],
            AmenityType::CarShare => vec!["car_sharing"],
            AmenityType::Childcare => vec!["childcare", "kindergarten"],
            AmenityType::ConvenienceStore => vec!["convenience"],
            AmenityType::Culture => vec!["arts_centre", "art", "cinema", "theatre"],
            AmenityType::Exercise => vec!["fitness_centre", "sports_centre", "track", "pitch"],
            AmenityType::FastFood => vec!["fast_food", "food_court"],
            AmenityType::Food => vec![
                "restaurant",
                "farm",
                "ice_cream",
                "seafood",
                "cheese",
                "chocolate",
                "deli",
                "butcher",
                "confectionery",
                "beverages",
                "alcohol",
            ],
            AmenityType::GreenSpace => vec!["park", "garden", "nature_reserve"],
            AmenityType::Hotel => vec!["hotel", "hostel", "guest_house", "motel"],
            AmenityType::Laundry => vec!["dry_cleaning", "laundry", "tailor"],
            AmenityType::Library => vec!["library"],
            AmenityType::Medical => vec![
                "clinic", "dentist", "hospital", "pharmacy", "doctors", "optician",
            ],
            AmenityType::Pet => vec!["veterinary", "pet", "animal_boarding", "pet_grooming"],
            AmenityType::Playground => vec!["playground"],
            AmenityType::Pool => vec!["swimming_pool"],
            AmenityType::PostOffice => vec!["post_office"],
            AmenityType::Religious => vec!["place_of_worship", "religion"],
            AmenityType::School => vec!["school"],
            AmenityType::Shopping => vec![
                "wholesale",
                "bag",
                "marketplace",
                "second_hand",
                "charity",
                "clothes",
                "lottery",
                "shoes",
                "mall",
                "department_store",
                "car",
                "tailor",
                "nutrition_supplements",
                "watches",
                "craft",
                "fabric",
                "kiosk",
                "antiques",
                "shoemaker",
                "hardware",
                "houseware",
                "mobile_phone",
                "photo",
                "toys",
                "bed",
                "florist",
                "electronics",
                "fishing",
                "garden_centre",
                "frame",
                "watchmaker",
                "boutique",
                "mobile_phone",
                "party",
                "car_parts",
                "video",
                "video_games",
                "musical_instrument",
                "music",
                "baby_goods",
                "doityourself",
                "jewelry",
                "variety_store",
                "gift",
                "carpet",
                "perfumery",
                "curtain",
                "appliance",
                "furniture",
                "lighting",
                "sewing",
                "books",
                "sports",
                "travel_agency",
                "interior_decoration",
                "stationery",
                "computer",
                "tyres",
                "newsagent",
                "general",
            ],
            AmenityType::Supermarket => vec!["supermarket", "greengrocer"],
            AmenityType::Tourism => vec![
                "gallery",
                "museum",
                "zoo",
                "attraction",
                "theme_park",
                "aquarium",
            ],
            AmenityType::University => vec!["college", "university"],
        }
    }

    /// All types of amenities, in alphabetical order.
    pub fn all() -> Vec<AmenityType> {
        AmenityType::iter().collect()
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
}

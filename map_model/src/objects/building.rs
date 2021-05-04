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
    Bar,
    Beauty,
    BikeStore,
    BikeParkRent,
    CarRepair,
    CarShare,
    Childcare,
    ConvenienceStore,
    Culture,
    Financial,
    Food,
    Laundry,
    Medical,
    Pet,
    PostOffice,
    Religious,
    School,
    Shopping,
    Supermarket,
    University,
}

impl AmenityType {
    fn types(self) -> Vec<&'static str> {
        match self {
            AmenityType::Bar => vec!["bar", "lounge", "pub", "nightclub", "biergarten"],
            AmenityType::Beauty => vec!["hairdresser", "beauty", "chemist", "cosmetics"],
            AmenityType::BikeParkRent => vec!["bicycle_parking", "bicycle_rental"],
            AmenityType::BikeStore => vec!["bicycle"],
            AmenityType::CarRepair => vec!["car_repair"],
            AmenityType::CarShare => vec!["car_sharing"],
            AmenityType::Childcare => vec!["childcare", "kindergarten"],
            AmenityType::ConvenienceStore => vec!["convenience"],
            AmenityType::Culture => vec![
                "arts_centre",
                "art_gallery",
                "cinema",
                "library",
                "museum",
                "theatre",
            ],
            AmenityType::Financial => vec!["bank"],
            AmenityType::Food => vec![
                "restaurant",
                "cafe",
                "farm",
                "fast_food",
                "food_court",
                "ice_cream",
                "pastry",
                "pasta",
                "spices",
                "seafood",
                "tea",
                "coffee",
                "cheese",
                "chocolate",
                "deli",
                "bakery",
                "butcher",
                "confectionery",
                "beverages",
                "alcohol",
            ],
            AmenityType::Laundry => vec!["dry_cleaning", "laundry", "tailor"],
            AmenityType::Medical => vec![
                "chiropractor",
                "clinic",
                "dentist",
                "hospital",
                "pharmacy",
                "doctors",
                "optician",
            ],
            AmenityType::Pet => vec!["veterinary", "pet", "animal_boarding", "pet_grooming"],
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
                "furniture",
                "shoes",
                "mall",
                "department_store",
                "car",
                "tailor",
                "nutrition_supplements",
                "watches",
                "craft",
                "wool",
                "fabric",
                "fashion_accessories",
                "kiosk",
                "antiques",
                "shoemaker",
                "ski",
                "hardware",
                "houseware",
                "mobile_phone",
                "weapons",
                "photo",
                "toys",
                "camera",
                "bed",
                "florist",
                "electronics",
                "fishing",
                "garden_centre",
                "garden_furniture",
                "collector",
                "frame",
                "watchmaker",
                "golf",
                "hunting",
                "boutique",
                "candles",
                "atv",
                "mobile_phone",
                "radiotechnics",
                "party",
                "car_parts",
                "vacuum_cleaner",
                "video",
                "video_games",
                "musical_instrument",
                "music",
                "art",
                "baby_goods",
                "doityourself",
                "jewelry",
                "leather",
                "variety_store",
                "gift",
                "carpet",
                "perfumery",
                "curtain",
                "appliance",
                "window_blind",
                "furniture",
                "lighting",
                "sewing",
                "household_linen",
                "books",
                "sports",
                "travel_agency",
                "interior_decoration",
                "stationery",
                "games",
                "computer",
                "tyres",
                "newsagent",
                "general"
            ],
            AmenityType::Supermarket => vec!["supermarket", "greengrocer"],
            AmenityType::University => vec!["college", "university"],
        }
    }

    /// All types of amenities, in an arbitrary order.
    pub fn all() -> Vec<AmenityType> {
        vec![
            AmenityType::Bar,
            AmenityType::Beauty,
            AmenityType::BikeStore,
            AmenityType::BikeParkRent,
            AmenityType::CarRepair,
            AmenityType::CarShare,
            AmenityType::Childcare,
            AmenityType::ConvenienceStore,
            AmenityType::Culture,
            AmenityType::Financial,
            AmenityType::Food,
            AmenityType::Laundry,
            AmenityType::Medical,
            AmenityType::Pet,
            AmenityType::PostOffice,
            AmenityType::Religious,
            AmenityType::School,
            AmenityType::Shopping,
            AmenityType::Supermarket,
            AmenityType::University,
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
            "bar" => Some(AmenityType::Bar),
            "beauty" => Some(AmenityType::Beauty),
            "bike store" => Some(AmenityType::BikeStore),
            "bike parking rental" => Some(AmenityType::BikeParkRent),
            "car repair" => Some(AmenityType::CarRepair),
            "car share" => Some(AmenityType::CarShare),
            "childcare" => Some(AmenityType::Childcare),
            "convenience store" => Some(AmenityType::ConvenienceStore),
            "culture" => Some(AmenityType::Culture),
            "financial" => Some(AmenityType::Financial),
            "food" => Some(AmenityType::Food),
            "laundry" => Some(AmenityType::Laundry),
            "medical" => Some(AmenityType::Medical),
            "pet" => Some(AmenityType::Pet),
            "post office" => Some(AmenityType::PostOffice),
            "religious" => Some(AmenityType::Religious),
            "school" => Some(AmenityType::School),
            "shopping" => Some(AmenityType::Shopping),
            "supermarket" => Some(AmenityType::Supermarket),
            "university" => Some(AmenityType::University),
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
                AmenityType::Bar => "bar",
                AmenityType::Beauty => "beauty",
                AmenityType::BikeStore => "bike store",
                AmenityType::BikeParkRent => "bike parking rental",
                AmenityType::CarRepair => "car repair",
                AmenityType::CarShare => "car share",
                AmenityType::Childcare => "childcare",
                AmenityType::ConvenienceStore => "convenience store",
                AmenityType::Culture => "culture",
                AmenityType::Financial => "financial",
                AmenityType::Food => "food",
                AmenityType::Laundry => "laundry",
                AmenityType::Medical => "medical",
                AmenityType::Pet => "pet",
                AmenityType::PostOffice => "post office",
                AmenityType::Religious => "religious",
                AmenityType::School => "school",
                AmenityType::Shopping => "shopping",
                AmenityType::Supermarket => "supermarket",
                AmenityType::University => "university",
            }
        )
    }
}



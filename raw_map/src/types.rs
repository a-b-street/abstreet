use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use abstutil::Tags;
use osm2streets::NamePerLanguage;

/// A business located inside a building.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Amenity {
    pub names: NamePerLanguage,
    /// This is the specific amenity listed in OSM, not the more general `AmenityType` category.
    pub amenity_type: String,
    /// Depending on options while importing, these might be empty, to save file space.
    pub osm_tags: Tags,
}

/// Businesses are categorized into one of these types.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumString, Display, EnumIter, Debug)]
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum AreaType {
    Park,
    Water,
    Island,
    /// Not from OSM. A user-specified area to focus on.
    StudyArea,
}

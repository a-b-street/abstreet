use abstio::{CityName, MapName};
use geom::Distance;
use map_model::DrivingSide;

/// Given the name of a map, configure its import.
///
/// Note this was once expressed as config files for every city. That was less maintainable; most
/// places used default values that were copied around.
// Slightly more verbose logic feels easier to read
#[allow(clippy::match_like_matches_macro)]
pub fn config_for_map(name: &MapName) -> convert_osm::Options {
    // Some maps have extra procedurally generated houses. Just see if a file in a canonical
    // location exists.
    let procgen_houses = name.city.input_path("procgen_houses.json");
    let extra_buildings = if abstio::file_exists(&procgen_houses) {
        Some(procgen_houses)
    } else {
        None
    };

    convert_osm::Options {
        map_config: map_model::MapConfig {
            driving_side: match name.city.country.as_ref() {
                "au" | "gb" | "in" | "jp" | "nz" | "sg" => DrivingSide::Left,
                _ => DrivingSide::Right,
            },
            bikes_can_use_bus_lanes: name.city.country != "pl",
            inferred_sidewalks: name.city.country != "pl",
            street_parking_spot_length: if name.city == CityName::new("ca", "montreal") {
                Distance::meters(6.5)
            } else {
                Distance::meters(8.0)
            },
            turn_on_red: name.city.country == "us" && name.city.city != "nyc",
        },
        onstreet_parking: match name.city.city.as_ref() {
            "seattle" => {
                convert_osm::OnstreetParking::Blockface(name.city.input_path("blockface.bin"))
            }
            "lyon" | "milwaukee" | "montreal" | "tel_aviv" | "zurich" => {
                convert_osm::OnstreetParking::SomeAdditionalWhereNoData { pct: 50 }
            }
            "krakow" | "warsaw" => {
                convert_osm::OnstreetParking::SomeAdditionalWhereNoData { pct: 90 }
            }
            _ => convert_osm::OnstreetParking::JustOSM,
        },
        public_offstreet_parking: if name.city == CityName::seattle() {
            convert_osm::PublicOffstreetParking::Gis(name.city.input_path("offstreet_parking.bin"))
        } else {
            convert_osm::PublicOffstreetParking::None
        },
        private_offstreet_parking: if name.city == CityName::seattle() {
            convert_osm::PrivateOffstreetParking::FixedPerBldg(
                // TODO Utter guesses or in response to gridlock
                match name.map.as_ref() {
                    "downtown" => 5,
                    "lakeslice" => 5,
                    "qa" => 5,
                    "south_seattle" => 5,
                    "wallingford" => 5,
                    _ => 1,
                },
            )
        } else {
            convert_osm::PrivateOffstreetParking::FixedPerBldg(3)
        },
        include_railroads: match name.city.city.as_ref() {
            "phoenix" | "seattle" | "tucson" => false,
            _ => true,
        },
        extra_buildings,
        skip_local_roads: name == &MapName::new("us", "phoenix", "loop101"),
        filter_crosswalks: false,
        // https://www.transit.land is a great place to find the static GTFS URLs
        gtfs_url: if name == &MapName::new("us", "seattle", "arboretum") {
            Some("http://metro.kingcounty.gov/GTFS/google_transit.zip".to_string())
        } else if name.city == CityName::new("us", "san_francisco") {
            Some("https://gtfs.sfmta.com/transitdata/google_transit.zip".to_string())
        } else if name.city == CityName::new("br", "sao_paulo") {
            Some("https://github.com/transitland/gtfs-archives-not-hosted-elsewhere/blob/master/sao-paulo-sptrans.zip?raw=true".to_string())
        } else {
            None
        },
        elevation: true,
    }
}

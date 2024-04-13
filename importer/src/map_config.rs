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
    convert_osm::Options {
        map_config: osm2streets::MapConfig {
            // osm2streets will set this anyway, it doesn't matter here
            driving_side: DrivingSide::Right,
            country_code: String::new(),
            bikes_can_use_bus_lanes: name.city.country != "pl",
            inferred_sidewalks: name.city.country != "pl",
            street_parking_spot_length: if name.city == CityName::new("ca", "montreal") {
                Distance::meters(6.5)
            } else {
                Distance::meters(8.0)
            },
            turn_on_red: name.city.country == "us" && name.city.city != "nyc",
            include_railroads: match name.city.city.as_ref() {
                "phoenix" | "seattle" | "tucson" => false,
                _ => {
                    if name.map == "hammersmith_and_fulham" {
                        // TODO Some movement geometry bug here
                        false
                    } else {
                        true
                    }
                }
            },
        },
        filter_crosswalks: false,
        onstreet_parking: match name.city.city.as_ref() {
            "seattle" => {
                convert_osm::OnstreetParking::Blockface(name.city.input_path("blockface.bin"))
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
        // Unused currently
        extra_buildings: None,
        // https://www.transit.land is a great place to find the static GTFS URLs
        gtfs_url: if name == &MapName::new("us", "seattle", "arboretum") {
            Some("http://metro.kingcounty.gov/GTFS/google_transit.zip".to_string())
        } else if name.city == CityName::new("us", "san_francisco") {
            None
            // Crashing the traffic sim, so disabled
            //Some("https://gtfs.sfmta.com/transitdata/google_transit.zip".to_string())
        } else if name == &MapName::new("br", "sao_paulo", "aricanduva") {
            Some("https://github.com/transitland/gtfs-archives-not-hosted-elsewhere/blob/master/sao-paulo-sptrans.zip?raw=true".to_string())
        } else if name.city == CityName::new("fr", "brest") {
            Some("https://ratpdev-mosaic-prod-bucket-raw.s3-eu-west-1.amazonaws.com/11/exports/1/gtfs.zip".to_string())
        } else {
            None
        },
        // We only have a few elevation sources working
        elevation_geotiff: if name.city == CityName::new("us", "seattle") {
            Some("data/input/shared/elevation/king_county_2016_lidar.tif".to_string())
        } else if name.city.country == "gb" {
            Some("data/input/shared/elevation/UK-dem-50m-4326.tif".to_string())
        } else {
            None
        },
    }
}

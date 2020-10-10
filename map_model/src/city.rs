use serde::{Deserialize, Serialize};

use geom::{LonLat, Polygon, Ring};

use crate::{AreaType, Map};

/// A single city (like Seattle) can be broken down into multiple boundary polygons (udistrict,
/// ballard, downtown, etc). The load map screen uses this struct to display the entire city.
#[derive(Serialize, Deserialize)]
pub struct City {
    pub name: String,
    pub boundary: Polygon,
    pub areas: Vec<(AreaType, Polygon)>,
    // The individual maps
    pub regions: Vec<(String, Polygon)>,
    // TODO Move nice_map_name from game into here?
}

impl City {
    pub fn new(huge_map: &Map) -> City {
        let mut regions = abstutil::list_all_objects(abstutil::path(format!(
            "input/{}/polygons",
            huge_map.get_city_name()
        )))
        .into_iter()
        .map(|name| {
            let pts = LonLat::read_osmosis_polygon(abstutil::path(format!(
                "input/{}/polygons/{}.poly",
                huge_map.get_city_name(),
                name
            )))
            .unwrap();
            (
                name,
                Ring::must_new(huge_map.get_gps_bounds().convert(&pts)).to_polygon(),
            )
        })
        .collect::<Vec<_>>();
        // Just a sort of z-ordering hack so that the largest encompassing region isn't first
        // later in the UI picker.
        regions.sort_by_key(|(_, poly)| poly.get_bounds().width() as usize);

        City {
            name: huge_map.get_city_name().to_string(),
            boundary: huge_map.get_boundary_polygon().clone(),
            areas: huge_map
                .all_areas()
                .iter()
                .map(|a| (a.area_type, a.polygon.clone()))
                .collect(),
            regions,
        }
    }
}

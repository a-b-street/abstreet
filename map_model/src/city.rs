use crate::{AreaType, Map};
use geom::{LonLat, Polygon, Ring};
use serde::{Deserialize, Serialize};

// TODO Ah we could also stash the friendly names here!
#[derive(Serialize, Deserialize)]
pub struct City {
    pub name: String,
    pub boundary: Polygon,
    pub areas: Vec<(AreaType, Polygon)>,
    pub regions: Vec<(String, Polygon)>,
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
            // TODO Maybe simplify it? :P
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

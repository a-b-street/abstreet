use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::Timer;
use geom::{GPSBounds, LonLat, Polygon, Ring};

use crate::{AreaType, Map};

/// A single city (like Seattle) can be broken down into multiple boundary polygons (udistrict,
/// ballard, downtown, etc). The load map screen uses this struct to display the entire city.
#[derive(Serialize, Deserialize)]
pub struct City {
    pub name: String,
    pub boundary: Polygon,
    pub areas: Vec<(AreaType, Polygon)>,
    /// The individual maps
    pub regions: Vec<(MapName, Polygon)>,
    // TODO Move nice_map_name from game into here?
}

impl City {
    /// If there's a single map covering all the smaller maps, use this.
    pub fn from_huge_map(huge_map: &Map) -> City {
        let city_name = huge_map.get_city_name().clone();
        let mut regions = abstio::list_dir(format!("importer/config/{}", city_name))
            .into_iter()
            .filter(|path| path.ends_with(".poly"))
            .map(|path| {
                let pts = LonLat::read_osmosis_polygon(&path).unwrap();
                (
                    MapName::new(&city_name, &abstutil::basename(path)),
                    Ring::must_new(huge_map.get_gps_bounds().convert(&pts)).to_polygon(),
                )
            })
            .collect::<Vec<_>>();
        // Just a sort of z-ordering hack so that the largest encompassing region isn't first
        // later in the UI picker.
        regions.sort_by_key(|(_, poly)| poly.get_bounds().width() as usize);

        City {
            name: city_name,
            boundary: huge_map.get_boundary_polygon().clone(),
            areas: huge_map
                .all_areas()
                .iter()
                .map(|a| (a.area_type, a.polygon.clone()))
                .collect(),
            regions,
        }
    }

    /// Generate a city from a bunch of smaller, individual maps. The boundaries of those maps
    /// may overlap and may have gaps between them.
    pub fn from_individual_maps(city_name: &str, timer: &mut Timer) -> City {
        let boundary_per_region: Vec<(MapName, Vec<LonLat>)> =
            abstio::list_dir(format!("importer/config/{}", city_name))
                .into_iter()
                .filter(|path| path.ends_with(".poly"))
                .map(|path| {
                    (
                        MapName::new(&city_name, &abstutil::basename(&path)),
                        LonLat::read_osmosis_polygon(&path).unwrap(),
                    )
                })
                .collect();
        // Figure out the total bounds for all the maps
        let mut gps_bounds = GPSBounds::new();
        for (_, pts) in &boundary_per_region {
            for pt in pts {
                gps_bounds.update(*pt);
            }
        }
        let boundary = gps_bounds.to_bounds().get_rectangle();

        let mut regions = Vec::new();
        for (name, pts) in boundary_per_region {
            regions.push((name, Ring::must_new(gps_bounds.convert(&pts)).to_polygon()));
        }

        // Add areas from every map. It's fine if they partly overlap.
        let mut areas = Vec::new();
        for path in abstio::list_dir(abstio::path(format!("system/{}/maps", city_name))) {
            let map = Map::new(path, timer);
            for area in map.all_areas() {
                let pts = map.gps_bounds.convert_back(area.polygon.points());
                // TODO Holes in the polygons get lost
                if let Ok(ring) = Ring::new(gps_bounds.convert(&pts)) {
                    areas.push((area.area_type, ring.to_polygon()));
                }
            }
        }

        City {
            name: city_name.to_string(),
            boundary,
            areas,
            regions,
        }
    }
}

use serde::{Deserialize, Serialize};

use abstio::{CityName, MapName};
use abstutil::Timer;
use geom::{GPSBounds, LonLat, Polygon, Ring};

use crate::{AreaType, Map};

/// A single city (like Seattle) can be broken down into multiple boundary polygons (udistrict,
/// ballard, downtown, etc). The load map screen uses this struct to display the entire city.
#[derive(Serialize, Deserialize)]
pub struct City {
    pub name: CityName,
    pub boundary: Polygon,
    pub areas: Vec<(AreaType, Polygon)>,
    /// The individual maps
    pub districts: Vec<(MapName, Polygon)>,
    // TODO Move nice_map_name from game into here?
}

impl City {
    /// If there's a single map covering all the smaller maps, use this.
    pub fn from_huge_map(huge_map: &Map) -> City {
        let city_name = huge_map.get_city_name().clone();
        let mut districts = abstio::list_dir(format!(
            "importer/config/{}/{}",
            city_name.country, city_name.city
        ))
        .into_iter()
        .filter(|path| path.ends_with(".poly"))
        .map(|path| {
            let pts = LonLat::read_osmosis_polygon(&path).unwrap();
            (
                MapName::from_city(&city_name, &abstutil::basename(path)),
                Ring::must_new(huge_map.get_gps_bounds().convert(&pts)).into_polygon(),
            )
        })
        .collect::<Vec<_>>();
        // Just a sort of z-ordering hack so that the largest encompassing district isn't first
        // later in the UI picker.
        districts.sort_by_key(|(_, poly)| poly.get_bounds().width() as usize);

        City {
            name: city_name,
            boundary: huge_map.get_boundary_polygon().clone(),
            areas: huge_map
                .all_areas()
                .iter()
                .map(|a| (a.area_type, a.polygon.clone()))
                .collect(),
            districts,
        }
    }

    /// Generate a city from a bunch of smaller, individual maps. The boundaries of those maps
    /// may overlap and may have gaps between them.
    pub fn from_individual_maps(city_name: &CityName, timer: &mut Timer) -> City {
        let boundary_per_district: Vec<(MapName, Vec<LonLat>)> = abstio::list_dir(format!(
            "importer/config/{}/{}",
            city_name.country, city_name.city
        ))
        .into_iter()
        .filter(|path| path.ends_with(".poly"))
        .map(|path| {
            (
                MapName::from_city(city_name, &abstutil::basename(&path)),
                LonLat::read_osmosis_polygon(&path).unwrap(),
            )
        })
        .collect();
        // Figure out the total bounds for all the maps
        let mut gps_bounds = GPSBounds::new();
        for (_, pts) in &boundary_per_district {
            for pt in pts {
                gps_bounds.update(*pt);
            }
        }
        let boundary = gps_bounds.to_bounds().get_rectangle();

        let mut districts = Vec::new();
        for (name, pts) in boundary_per_district {
            districts.push((
                name,
                Ring::must_new(gps_bounds.convert(&pts)).into_polygon(),
            ));
        }
        // Just a sort of z-ordering hack so that the largest encompassing district isn't first
        // later in the UI picker.
        districts.sort_by_key(|(_, poly)| poly.get_bounds().width() as usize);

        // Add areas from every map. It's fine if they partly overlap.
        let mut areas = Vec::new();
        for path in abstio::list_dir(abstio::path(format!(
            "system/{}/{}/maps",
            city_name.country, city_name.city
        ))) {
            let map = Map::load_synchronously(path, timer);
            for area in map.all_areas() {
                let pts = map.gps_bounds.convert_back(area.polygon.points());
                // TODO Holes in the polygons get lost
                if let Ok(ring) = Ring::new(gps_bounds.convert(&pts)) {
                    areas.push((area.area_type, ring.into_polygon()));
                }
            }
        }

        City {
            name: city_name.clone(),
            boundary,
            areas,
            districts,
        }
    }
}

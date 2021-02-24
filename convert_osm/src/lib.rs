#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use anyhow::Result;

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{Distance, FindClosest, GPSBounds, LonLat, PolyLine, Pt2D, Ring};
use map_model::raw::RawMap;
use map_model::{osm, raw, Amenity, MapConfig};
use serde::{Deserialize, Serialize};

mod clip;
mod extract;
pub mod osm_geom;
mod parking;
pub mod reader;
mod snappy;
mod split_ways;
mod srtm;
mod transit;

pub struct Options {
    pub osm_input: String,
    pub name: MapName,

    /// The path to an osmosis boundary polygon. Highly recommended.
    pub clip: Option<String>,
    pub map_config: MapConfig,

    pub onstreet_parking: OnstreetParking,
    pub public_offstreet_parking: PublicOffstreetParking,
    pub private_offstreet_parking: PrivateOffstreetParking,
    /// If provided, pull elevation data from this SRTM file. The SRTM parser is incorrect, so the
    /// results will be nonsense.
    pub elevation: Option<String>,
    /// OSM railway=rail will be included as light rail if so. Cosmetic only.
    pub include_railroads: bool,
    /// If provided, read polygons from this GeoJSON file and add them to the RawMap as buildings.
    pub extra_buildings: Option<String>,
}

/// What roads will have on-street parking lanes? Data from
/// <https://wiki.openstreetmap.org/wiki/Key:parking:lane> is always used if available.
#[derive(Clone, Serialize, Deserialize)]
pub enum OnstreetParking {
    /// If not tagged, there won't be parking.
    JustOSM,
    /// If OSM data is missing, then try to match data from
    /// <http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface>. This is Seattle specific.
    Blockface(String),
    /// If OSM data is missing, then infer parking lanes on some percentage of
    /// "highway=residential" roads.
    SomeAdditionalWhereNoData {
        /// [0, 100]
        pct: usize,
    },
}

/// How many spots are available in public parking garages?
#[derive(Clone, Serialize, Deserialize)]
pub enum PublicOffstreetParking {
    None,
    /// Pull data from
    /// <https://data-seattlecitygis.opendata.arcgis.com/datasets/public-garages-or-parking-lots>, a
    /// Seattle-specific data source.
    GIS(String),
}

/// If a building doesn't have anything from public_offstreet_parking and isn't tagged as a garage
/// in OSM, how many private spots should it have?
#[derive(Clone, Serialize, Deserialize)]
pub enum PrivateOffstreetParking {
    FixedPerBldg(usize),
    // TODO Based on the number of residents?
}

pub fn convert(opts: Options, timer: &mut abstutil::Timer) -> RawMap {
    let mut map = RawMap::blank(opts.name.clone());
    if let Some(ref path) = opts.clip {
        let pts = LonLat::read_osmosis_polygon(path).unwrap();
        let gps_bounds = GPSBounds::from(pts.clone());
        map.boundary_polygon = Ring::must_new(gps_bounds.convert(&pts)).to_polygon();
        map.gps_bounds = gps_bounds;
    }

    let extract = extract::extract_osm(&mut map, &opts, timer);
    let (amenities, pt_to_road) = split_ways::split_up_roads(&mut map, extract, timer);
    clip::clip_map(&mut map, timer);

    // Need to do a first pass of removing cul-de-sacs here, or we wind up with loop PolyLines when
    // doing the parking hint matching.
    abstutil::retain_btreemap(&mut map.roads, |r, _| r.i1 != r.i2);

    let all_routes = map.bus_routes.drain(..).collect::<Vec<_>>();
    let mut routes = Vec::new();
    for route in all_routes {
        let name = format!("{} ({})", route.osm_rel_id, route.full_name);
        match transit::snap_bus_stops(route, &mut map, &pt_to_road) {
            Ok(r) => {
                routes.push(r);
            }
            Err(err) => {
                error!("Skipping {}: {}", name, err);
            }
        }
    }
    map.bus_routes = routes;

    use_amenities(&mut map, amenities, timer);

    parking::apply_parking(&mut map, &opts, timer);

    if let Some(ref path) = opts.elevation {
        use_elevation(&mut map, path, timer);
    }
    if false {
        generate_elevation_queries(&map).unwrap();
    }
    if let Some(ref path) = opts.extra_buildings {
        add_extra_buildings(&mut map, path).unwrap();
    }

    snappy::snap_cycleways(&mut map, timer);

    map.config = opts.map_config;
    map
}

fn use_amenities(map: &mut RawMap, amenities: Vec<(Pt2D, Amenity)>, timer: &mut Timer) {
    let mut closest: FindClosest<osm::OsmID> = FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, b) in &map.buildings {
        closest.add(*id, b.polygon.points());
    }

    timer.start_iter("match building amenities", amenities.len());
    for (pt, amenity) in amenities {
        timer.next();
        if let Some((id, _)) = closest.closest_pt(pt, Distance::meters(50.0)) {
            let b = map.buildings.get_mut(&id).unwrap();
            if b.polygon.contains_pt(pt) {
                b.amenities.push(amenity);
            }
        }
    }
}

fn use_elevation(map: &mut RawMap, path: &str, timer: &mut Timer) {
    timer.start("apply elevation data to intersections");
    let elevation = srtm::Elevation::load(path).unwrap();
    for i in map.intersections.values_mut() {
        // TODO Not sure why, but I've seen nodes from South Carolina wind up in the updated
        // Seattle extract. And I think there's a bug with clipping, because they survive to this
        // point. O_O
        if map.boundary_polygon.contains_pt(i.point) {
            i.elevation = elevation.get(i.point.to_gps(&map.gps_bounds));
        }
    }
    timer.stop("apply elevation data to intersections");
}

fn generate_elevation_queries(map: &RawMap) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut f = File::create("elevation_queries")?;
    for r in map.roads.values() {
        // TODO Handle cul-de-sacs
        if let Ok(pl) = PolyLine::new(r.center_points.clone()) {
            // Sample points every meter along the road
            let mut pts = Vec::new();
            let mut dist = Distance::ZERO;
            while dist <= pl.length() {
                let (pt, _) = pl.dist_along(dist).unwrap();
                pts.push(pt);
                dist += Distance::meters(1.0);
            }
            // Always ask for the intersection
            if *pts.last().unwrap() != pl.last_pt() {
                pts.push(pl.last_pt());
            }
            for (idx, gps) in map.gps_bounds.convert_back(&pts).into_iter().enumerate() {
                write!(f, "{},{}", gps.x(), gps.y())?;
                if idx != pts.len() - 1 {
                    write!(f, ",")?;
                }
            }
            writeln!(f)?;
        }
    }
    Ok(())
}

fn add_extra_buildings(map: &mut RawMap, path: &str) -> Result<()> {
    // TODO Refactor code that just extracts polygons from geojson.
    let mut polygons = Vec::new();

    let bytes = abstio::slurp_file(path)?;
    let raw_string = std::str::from_utf8(&bytes)?;
    let geojson = raw_string.parse::<geojson::GeoJson>()?;

    if let geojson::GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            if let Some(geom) = feature.geometry {
                if let geojson::Value::Polygon(raw_pts) = geom.value {
                    // TODO Handle holes, and also, refactor this!
                    let gps_pts: Vec<LonLat> = raw_pts[0]
                        .iter()
                        .map(|pt| LonLat::new(pt[0], pt[1]))
                        .collect();
                    if let Some(pts) = map.gps_bounds.try_convert(&gps_pts) {
                        if let Ok(ring) = Ring::new(pts) {
                            polygons.push(ring.to_polygon());
                        }
                    }
                }
            }
        }
    }

    // Add these as new buildings, generating a new dummy OSM ID.
    let mut id = -1;
    for polygon in polygons {
        map.buildings.insert(
            osm::OsmID::Way(osm::WayID(id)),
            raw::RawBuilding {
                polygon,
                osm_tags: Tags::empty(),
                public_garage_name: None,
                num_parking_spots: 1,
                amenities: Vec::new(),
            },
        );
        // We could use new_osm_way_id, but faster to just assume we're the only place introducing
        // new OSM IDs.
        id -= -1;
    }

    Ok(())
}

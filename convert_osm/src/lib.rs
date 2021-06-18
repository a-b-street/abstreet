#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use anyhow::Result;

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{Distance, FindClosest, GPSBounds, LonLat, Polygon, Pt2D, Ring};
use map_model::raw::RawMap;
use map_model::{osm, raw, Amenity, MapConfig};
use serde::{Deserialize, Serialize};

mod clip;
mod elevation;
mod extract;
pub mod osm_geom;
mod parking;
pub mod reader;
mod snappy;
mod split_ways;
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
    /// OSM railway=rail will be included as light rail if so. Cosmetic only.
    pub include_railroads: bool,
    /// If provided, read polygons from this GeoJSON file and add them to the RawMap as buildings.
    pub extra_buildings: Option<String>,
    /// The path to an unzipped GTFS directory for public transit routes
    pub gtfs: Option<String>,
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
    Gis(String),
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
        map.boundary_polygon = Ring::must_new(gps_bounds.convert(&pts)).into_polygon();
        map.gps_bounds = gps_bounds;
    }

    let extract = extract::extract_osm(&mut map, &opts, timer);
    let (amenities, pt_to_road) = split_ways::split_up_roads(&mut map, extract, timer);
    clip::clip_map(&mut map, timer);

    // Need to do a first pass of removing cul-de-sacs here, or we wind up with loop PolyLines when
    // doing the parking hint matching.
    map.roads.retain(|r, _| r.i1 != r.i2);

    use_amenities(&mut map, amenities, timer);

    parking::apply_parking(&mut map, &opts, timer);

    // TODO Make this bail out on failure, after the new dependencies are clearly explained.
    timer.start("add elevation data");
    if let Err(err) = elevation::add_data(&mut map) {
        error!("No elevation data: {}", err);
    }
    timer.stop("add elevation data");
    if let Some(ref path) = opts.extra_buildings {
        add_extra_buildings(&mut map, path).unwrap();
    }

    snappy::snap_cycleways(&map, timer);

    if let Some(ref path) = opts.gtfs {
        transit::import_gtfs(&mut map, path).unwrap();
    }

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

fn add_extra_buildings(map: &mut RawMap, path: &str) -> Result<()> {
    let require_in_bounds = true;
    let mut id = -1;
    for (polygon, _) in Polygon::from_geojson_bytes(
        &abstio::slurp_file(path)?,
        &map.gps_bounds,
        require_in_bounds,
    )? {
        // Add these as new buildings, generating a new dummy OSM ID.
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

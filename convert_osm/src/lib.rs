mod clip;
mod osm_reader;
mod parking;
mod split_ways;
mod srtm;

use abstutil::Timer;
use geom::{Distance, FindClosest, Pt2D};
use map_model::raw::{OriginalBuilding, RawMap};
use map_model::MapConfig;

pub struct Options {
    pub osm_input: String,
    pub city_name: String,
    pub name: String,

    // The path to an osmosis boundary polygon. Highly recommended.
    pub clip: Option<String>,
    pub map_config: MapConfig,

    pub onstreet_parking: OnstreetParking,
    pub public_offstreet_parking: PublicOffstreetParking,
    pub private_offstreet_parking: PrivateOffstreetParking,
    // If provided, pull elevation data from this SRTM file. The SRTM parser is incorrect, so the
    // results will be nonsense.
    pub elevation: Option<String>,
    // OSM railway=rail will be included as light rail if so. Cosmetic only.
    pub include_railroads: bool,
}

// What roads will have on-street parking lanes? Data from
// https://wiki.openstreetmap.org/wiki/Key:parking:lane is always used if available.
pub enum OnstreetParking {
    // If not tagged, there won't be parking.
    JustOSM,
    // If OSM data is missing, then try to match data from
    // http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface. This is Seattle specific.
    Blockface(String),
    // If OSM data is missing, then infer parking lanes on some percentage of
    // "highway=residential" roads.
    SomeAdditionalWhereNoData {
        // [0, 100]
        pct: usize,
    },
}

// How many spots are available in public parking garages?
pub enum PublicOffstreetParking {
    None,
    // Pull data from
    // https://data-seattlecitygis.opendata.arcgis.com/datasets/public-garages-or-parking-lots, a
    // Seattle-specific data source.
    GIS(String),
}

// If a building doesn't have anything from public_offstreet_parking, how many private spots should
// it have?
pub enum PrivateOffstreetParking {
    FixedPerBldg(usize),
    // TODO Based on the number of residents?
}

pub fn convert(opts: Options, timer: &mut abstutil::Timer) -> RawMap {
    let (mut map, amenities) =
        split_ways::split_up_roads(osm_reader::extract_osm(&opts, timer), timer);
    clip::clip_map(&mut map, timer);

    // Need to do a first pass of removing cul-de-sacs here, or we wind up with loop PolyLines when
    // doing the parking hint matching.
    abstutil::retain_btreemap(&mut map.roads, |r, _| r.i1 != r.i2);

    use_amenities(&mut map, amenities, timer);

    parking::apply_parking(&mut map, &opts, timer);

    if let Some(ref path) = opts.elevation {
        use_elevation(&mut map, path, timer);
    }

    map.config = opts.map_config;
    map
}

fn use_amenities(map: &mut RawMap, amenities: Vec<(Pt2D, String, String)>, timer: &mut Timer) {
    let mut closest: FindClosest<OriginalBuilding> = FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, b) in &map.buildings {
        closest.add(*id, b.polygon.points());
    }

    timer.start_iter("match building amenities", amenities.len());
    for (pt, name, amenity) in amenities {
        timer.next();
        if let Some((id, _)) = closest.closest_pt(pt, Distance::meters(50.0)) {
            let b = map.buildings.get_mut(&id).unwrap();
            if b.polygon.contains_pt(pt) {
                b.amenities.insert((name, amenity));
            }
        }
    }
}

fn use_elevation(map: &mut RawMap, path: &str, timer: &mut Timer) {
    timer.start("apply elevation data to intersections");
    let elevation = srtm::Elevation::load(path).unwrap();
    for i in map.intersections.values_mut() {
        i.elevation = elevation.get(i.point.to_gps(&map.gps_bounds));
    }
    timer.stop("apply elevation data to intersections");
}

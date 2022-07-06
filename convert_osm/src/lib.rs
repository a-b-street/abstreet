#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{Distance, FindClosest, GPSBounds, HashablePt2D, LonLat, PolyLine, Polygon, Pt2D, Ring};
use raw_map::{osm, Amenity, MapConfig, OriginalRoad, RawMap, RawRoad};

mod clip;
mod elevation;
mod extract;
mod gtfs;
pub mod osm_geom;
mod parking;
pub mod reader;
mod split_ways;

/// Configures the creation of a RawMap from OSM and other input data.
pub struct Options {
    pub map_config: MapConfig,

    pub onstreet_parking: OnstreetParking,
    pub public_offstreet_parking: PublicOffstreetParking,
    pub private_offstreet_parking: PrivateOffstreetParking,
    /// OSM railway=rail will be included as light rail if so. Cosmetic only.
    pub include_railroads: bool,
    /// If provided, read polygons from this GeoJSON file and add them to the RawMap as buildings.
    pub extra_buildings: Option<String>,
    /// Only include highways and arterials. This may make sense for some region-wide maps for
    /// particular use cases.
    pub skip_local_roads: bool,
    /// Only include crosswalks that match a `highway=crossing` OSM node.
    pub filter_crosswalks: bool,
    /// Configure public transit using this URL to a static GTFS feed in .zip format.
    pub gtfs_url: Option<String>,
    pub elevation: bool,
}

impl Options {
    pub fn default_for_side(driving_side: raw_map::DrivingSide) -> Self {
        Self {
            map_config: MapConfig::default_for_side(driving_side),
            onstreet_parking: OnstreetParking::JustOSM,
            public_offstreet_parking: PublicOffstreetParking::None,
            private_offstreet_parking: PrivateOffstreetParking::FixedPerBldg(1),
            include_railroads: true,
            extra_buildings: None,
            skip_local_roads: false,
            filter_crosswalks: false,
            gtfs_url: None,
            elevation: false,
        }
    }
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

/// Create a RawMap from OSM and other input data.
pub fn convert(
    osm_input_path: String,
    name: MapName,
    clip_path: Option<String>,
    opts: Options,
    timer: &mut Timer,
) -> RawMap {
    let mut map = RawMap::blank(name);
    // Do this early. Calculating RawRoads uses DrivingSide, for example!
    map.streets.config = opts.map_config.clone();

    if let Some(ref path) = clip_path {
        let pts = LonLat::read_osmosis_polygon(path).unwrap();
        let gps_bounds = GPSBounds::from(pts.clone());
        map.streets.boundary_polygon = Ring::must_new(gps_bounds.convert(&pts)).into_polygon();
        map.streets.gps_bounds = gps_bounds;
    }

    let extract = extract::extract_osm(&mut map, &osm_input_path, clip_path, &opts, timer);
    let split_output = split_ways::split_up_roads(&mut map, extract, timer);
    clip::clip_map(&mut map, timer);

    // Need to do a first pass of removing cul-de-sacs here, or we wind up with loop PolyLines when
    // doing the parking hint matching.
    map.streets.roads.retain(|r, _| r.i1 != r.i2);

    use_amenities(&mut map, split_output.amenities, timer);

    parking::apply_parking(&mut map, &opts, timer);

    use_barrier_nodes(
        &mut map,
        split_output.barrier_nodes,
        &split_output.pt_to_road,
    );

    if opts.elevation {
        timer.start("add elevation data");
        if let Err(err) = elevation::add_data(&mut map) {
            error!("No elevation data: {}", err);
        }
        timer.stop("add elevation data");
    }
    if let Some(ref path) = opts.extra_buildings {
        add_extra_buildings(&mut map, path).unwrap();
    }

    if opts.filter_crosswalks {
        filter_crosswalks(
            &mut map,
            split_output.crosswalks,
            split_output.pt_to_road,
            timer,
        );
    }

    if opts.gtfs_url.is_some() {
        gtfs::import(&mut map).unwrap();
    }

    if map.name == MapName::new("gb", "bristol", "east") {
        bristol_hack(&mut map);
    }
    map
}

fn use_amenities(map: &mut RawMap, amenities: Vec<(Pt2D, Amenity)>, timer: &mut Timer) {
    let mut closest: FindClosest<osm::OsmID> =
        FindClosest::new(&map.streets.gps_bounds.to_bounds());
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
        &map.streets.gps_bounds,
        require_in_bounds,
    )? {
        // Add these as new buildings, generating a new dummy OSM ID.
        map.buildings.insert(
            osm::OsmID::Way(osm::WayID(id)),
            raw_map::RawBuilding {
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

fn filter_crosswalks(
    map: &mut RawMap,
    crosswalks: HashSet<HashablePt2D>,
    pt_to_road: HashMap<HashablePt2D, OriginalRoad>,
    timer: &mut Timer,
) {
    // Normally we assume every road has a crosswalk, but since this map is configured to use OSM
    // crossing nodes, let's reverse that assumption.
    for road in map.streets.roads.values_mut() {
        road.crosswalk_forward = false;
        road.crosswalk_backward = false;
    }

    // Match each crosswalk node to a road
    timer.start_iter("filter crosswalks", crosswalks.len());
    for pt in crosswalks {
        timer.next();
        // Some crossing nodes are outside the map boundary or otherwise not on a road that we
        // retained
        if let Some(road) = pt_to_road
            .get(&pt)
            .and_then(|r| map.streets.roads.get_mut(r))
        {
            // TODO Support cul-de-sacs and other loop roads
            if let Ok(pl) = PolyLine::new(road.osm_center_points.clone()) {
                // Crossings aren't right at an intersection. Where is this point along the center
                // line?
                if let Some((dist, _)) = pl.dist_along_of_point(pt.to_pt2d()) {
                    let pct = dist / pl.length();
                    // Don't throw away any crossings. If it occurs in the first half of the road,
                    // snap to the first intersection. If there's a mid-block crossing mapped,
                    // that'll likely not be correctly interpreted, unless an intersection is there
                    // anyway.
                    if pct <= 0.5 {
                        road.crosswalk_backward = true;
                    } else {
                        road.crosswalk_forward = true;
                    }

                    // TODO Some crosswalks incorrectly snap to the intersection near a short
                    // service road, which later gets trimmed. So the crosswalk effectively
                    // disappears.
                }
            }
        }
    }
}

fn use_barrier_nodes(
    map: &mut RawMap,
    barrier_nodes: HashSet<HashablePt2D>,
    pt_to_road: &HashMap<HashablePt2D, OriginalRoad>,
) {
    for pt in barrier_nodes {
        // Many barriers are on footpaths or roads that we don't retain
        if let Some(road) = pt_to_road
            .get(&pt)
            .and_then(|r| map.streets.roads.get_mut(r))
        {
            // Filters on roads that're already car-free are redundant
            if road.is_driveable() {
                road.barrier_nodes.push(pt.to_pt2d());
            }
        }
    }
}

// We're using Bristol for a project that requires an unusual LTN neighborhood boundary. Insert a
// fake road where a bridge crosses another road, to force blockfinding to trace along there.
fn bristol_hack(map: &mut RawMap) {
    let osm_way_id = map.new_osm_way_id(-1);
    let i1 = osm::NodeID(364061012);
    let i2 = osm::NodeID(1215755208);
    let id = OriginalRoad { osm_way_id, i1, i2 };
    let mut tags = Tags::empty();
    tags.insert("highway", "service");
    tags.insert("name", "Fake road");
    tags.insert("oneway", "yes");
    tags.insert("sidewalk", "none");
    tags.insert("lanes", "1");
    // TODO The LTN pathfinding tool will try to use this road. Discourage that heavily. It'd be
    // safer to mark this as under construction, but then blockfinding wouldn't treat it as a
    // boundary.
    tags.insert("maxspeed", "1 mph");
    tags.insert("bicycle", "no");

    map.streets.roads.insert(
        id,
        RawRoad::new(
            vec![
                map.streets.intersections[&i1].point,
                map.streets.intersections[&i2].point,
            ],
            tags,
            &map.streets.config,
        )
        .unwrap(),
    );
}

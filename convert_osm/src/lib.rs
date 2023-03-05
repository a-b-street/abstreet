#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use std::collections::{HashMap, HashSet};

use anyhow::Result;

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{Distance, HashablePt2D, LonLat, PolyLine, Polygon};
use osm2streets::{osm, MapConfig, Road, RoadID};
use raw_map::{CrossingType, ExtraRoadData, RawMap};

mod elevation;
mod extract;
mod gtfs;
mod parking;

/// Configures the creation of a `RawMap` from OSM and other input data.
pub struct Options {
    pub map_config: MapConfig,

    pub onstreet_parking: OnstreetParking,
    pub public_offstreet_parking: PublicOffstreetParking,
    pub private_offstreet_parking: PrivateOffstreetParking,
    /// If provided, read polygons from this GeoJSON file and add them to the RawMap as buildings.
    pub extra_buildings: Option<String>,
    /// Configure public transit using this URL to a static GTFS feed in .zip format.
    pub gtfs_url: Option<String>,
    pub elevation: bool,
    /// Only include crosswalks that match a `highway=crossing` OSM node.
    pub filter_crosswalks: bool,
}

impl Options {
    pub fn default() -> Self {
        Self {
            map_config: MapConfig::default(),
            onstreet_parking: OnstreetParking::JustOSM,
            public_offstreet_parking: PublicOffstreetParking::None,
            private_offstreet_parking: PrivateOffstreetParking::FixedPerBldg(1),
            extra_buildings: None,
            gtfs_url: None,
            elevation: false,
            filter_crosswalks: false,
        }
    }
}

/// What roads will have on-street parking lanes? Data from
/// <https://wiki.openstreetmap.org/wiki/Key:parking:lane> is always used if available.
pub enum OnstreetParking {
    /// If not tagged, there won't be parking.
    JustOSM,
    /// If OSM data is missing, then try to match data from
    /// <http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface>. This is Seattle specific.
    Blockface(String),
}

/// How many spots are available in public parking garages?
pub enum PublicOffstreetParking {
    None,
    /// Pull data from
    /// <https://data-seattlecitygis.opendata.arcgis.com/datasets/public-garages-or-parking-lots>, a
    /// Seattle-specific data source.
    Gis(String),
}

/// If a building doesn't have anything from public_offstreet_parking and isn't tagged as a garage
/// in OSM, how many private spots should it have?
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
    timer.start("create RawMap from input data");

    let mut map = RawMap::blank(name);
    // Note that DrivingSide is still incorrect. It'll be set in extract_osm, before Road::new
    // happens in split_ways.
    map.streets.config = opts.map_config.clone();

    let clip_pts = clip_path.map(|path| LonLat::read_geojson_polygon(&path).unwrap());
    timer.start("extract all from OSM");
    let extract = extract::extract_osm(&mut map, &osm_input_path, clip_pts, &opts, timer);
    timer.stop("extract all from OSM");
    let pt_to_road =
        streets_reader::split_ways::split_up_roads(&mut map.streets, extract.osm, timer);

    // Cul-de-sacs aren't supported yet.
    map.streets.retain_roads(|r| r.src_i != r.dst_i);

    map.bus_routes_on_roads = extract.bus_routes_on_roads;

    clip_map(&mut map, timer);

    for i in map.streets.intersections.keys() {
        map.elevation_per_intersection.insert(*i, Distance::ZERO);
    }
    for r in map.streets.roads.keys() {
        map.extra_road_data.insert(*r, ExtraRoadData::default());
    }

    // Remember OSM tags for all roads. Do this before apply_parking, which looks at tags
    timer.start("preserve OSM tags");
    let mut way_ids = HashSet::new();
    for r in map.streets.roads.values() {
        for id in &r.osm_ids {
            way_ids.insert(*id);
        }
    }
    for (id, way) in extract.doc.ways {
        if way_ids.contains(&id) {
            map.osm_tags.insert(id, way.tags);
        }
    }
    timer.stop("preserve OSM tags");

    parking::apply_parking(&mut map, &opts, timer);

    timer.start("use barrier and crossing nodes");
    use_barrier_nodes(&mut map, extract.barrier_nodes, &pt_to_road);
    use_crossing_nodes(&mut map, &extract.crossing_nodes, &pt_to_road);
    timer.stop("use barrier and crossing nodes");

    if opts.filter_crosswalks {
        filter_crosswalks(&mut map, extract.crossing_nodes, pt_to_road, timer);
    }

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

    if opts.gtfs_url.is_some() {
        gtfs::import(&mut map).unwrap();
    }

    if map.name == MapName::new("gb", "bristol", "east") {
        bristol_hack(&mut map);
    }

    timer.stop("create RawMap from input data");

    map
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

// We're using Bristol for a project that requires an unusual LTN neighborhood boundary. Insert a
// fake road where a bridge crosses another road, to force blockfinding to trace along there.
fn bristol_hack(map: &mut RawMap) {
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

    let src_i = map
        .streets
        .intersections
        .values()
        .find(|i| i.osm_ids.contains(&osm::NodeID(364061012)))
        .unwrap()
        .id;
    let dst_i = map
        .streets
        .intersections
        .values()
        .find(|i| i.osm_ids.contains(&osm::NodeID(1215755208)))
        .unwrap()
        .id;

    let id = map.streets.next_road_id();
    map.streets.insert_road(Road::new(
        id,
        Vec::new(),
        src_i,
        dst_i,
        PolyLine::must_new(vec![
            map.streets.intersections[&src_i].polygon.center(),
            map.streets.intersections[&dst_i].polygon.center(),
        ]),
        tags,
        &map.streets.config,
    ));
    map.extra_road_data.insert(id, ExtraRoadData::default());
}

fn clip_map(map: &mut RawMap, timer: &mut Timer) {
    let boundary_polygon = map.streets.boundary_polygon.clone();

    map.buildings = timer.retain_parallelized(
        "clip buildings to boundary",
        std::mem::take(&mut map.buildings),
        |b| {
            b.polygon
                .get_outer_ring()
                .points()
                .iter()
                .all(|pt| boundary_polygon.contains_pt(*pt))
        },
    );

    map.areas = timer
        .parallelize(
            "clip areas to boundary",
            std::mem::take(&mut map.areas),
            |orig_area| {
                let mut result = Vec::new();
                // If clipping fails, giving up on some areas is fine
                if let Ok(list) = map
                    .streets
                    .boundary_polygon
                    .intersection(&orig_area.polygon)
                {
                    for polygon in list {
                        let mut area = orig_area.clone();
                        area.polygon = polygon;
                        result.push(area);
                    }
                }
                result
            },
        )
        .into_iter()
        .flatten()
        .collect();

    // TODO Don't touch parking lots. It'll be visually obvious if a clip intersects one of these.
    // The boundary should be manually adjusted.
}

fn use_barrier_nodes(
    map: &mut RawMap,
    barrier_nodes: Vec<(osm::NodeID, HashablePt2D)>,
    pt_to_road: &HashMap<HashablePt2D, RoadID>,
) {
    // An OSM node likely only maps to one intersection
    let mut node_to_intersection = HashMap::new();
    for i in map.streets.intersections.values() {
        for node in &i.osm_ids {
            node_to_intersection.insert(*node, i.id);
        }
    }

    for (node, pt) in barrier_nodes {
        // Many barriers are on footpaths or roads that we don't retain
        if let Some(road) = pt_to_road.get(&pt).and_then(|r| map.streets.roads.get(r)) {
            // Filters on roads that're already car-free are redundant
            if road.is_driveable() {
                map.extra_road_data
                    .get_mut(&road.id)
                    .unwrap()
                    .barrier_nodes
                    .push(pt.to_pt2d());
            }
        } else if let Some(i) = node_to_intersection.get(&node) {
            let roads = &map.streets.intersections[i].roads;
            if roads.len() == 2 {
                // Arbitrarily put the barrier on one of the roads
                map.extra_road_data
                    .get_mut(&roads[0])
                    .unwrap()
                    .barrier_nodes
                    .push(pt.to_pt2d());
            } else {
                // TODO Look for real examples at non-2-way intersections to understand what to do.
                // If there's a barrier in the middle of a 4-way, does that disconnect all
                // movements?
                warn!(
                    "There's a barrier at {i}, but there are {} roads connected",
                    roads.len()
                );
            }
        }
    }
}

fn use_crossing_nodes(
    map: &mut RawMap,
    crossing_nodes: &HashSet<(HashablePt2D, CrossingType)>,
    pt_to_road: &HashMap<HashablePt2D, RoadID>,
) {
    for (pt, kind) in crossing_nodes {
        // Some crossings are on footpaths or roads that we don't retain
        if let Some(road) = pt_to_road
            .get(pt)
            .and_then(|r| map.extra_road_data.get_mut(r))
        {
            road.crossing_nodes.push((pt.to_pt2d(), *kind));
        }
    }
}

fn filter_crosswalks(
    map: &mut RawMap,
    crosswalks: HashSet<(HashablePt2D, CrossingType)>,
    pt_to_road: HashMap<HashablePt2D, RoadID>,
    timer: &mut Timer,
) {
    // Normally we assume every road has a crosswalk, but since this map is configured to use OSM
    // crossing nodes, let's reverse that assumption.
    for road in map.extra_road_data.values_mut() {
        road.crosswalk_forward = false;
        road.crosswalk_backward = false;
    }

    // Match each crosswalk node to a road
    timer.start_iter("filter crosswalks", crosswalks.len());
    for (pt, _) in crosswalks {
        timer.next();
        // Some crossing nodes are outside the map boundary or otherwise not on a road that we
        // retained
        if let Some(road) = pt_to_road.get(&pt).and_then(|r| map.streets.roads.get(r)) {
            // Crossings aren't right at an intersection. Where is this point along the center
            // line?
            if let Some((dist, _)) = road.reference_line.dist_along_of_point(pt.to_pt2d()) {
                let pct = dist / road.reference_line.length();
                // Don't throw away any crossings. If it occurs in the first half of the road, snap
                // to the first intersection. If there's a mid-block crossing mapped, that'll
                // likely not be correctly interpreted, unless an intersection is there anyway.
                let data = map.extra_road_data.get_mut(&road.id).unwrap();
                if pct <= 0.5 {
                    data.crosswalk_backward = true;
                } else {
                    data.crosswalk_forward = true;
                }

                // TODO Some crosswalks incorrectly snap to the intersection near a short service
                // road, which later gets trimmed. So the crosswalk effectively disappears.
            }
        }
    }
}

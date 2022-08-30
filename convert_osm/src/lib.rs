#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use anyhow::Result;

use abstio::MapName;
use abstutil::{Tags, Timer};
use geom::{Distance, FindClosest, GPSBounds, LonLat, Polygon, Pt2D, Ring};
use raw_map::{Amenity, RawMap};
use street_network::{osm, OriginalRoad, Road};

pub use import_streets::{
    OnstreetParking, Options, PrivateOffstreetParking, PublicOffstreetParking,
};

mod elevation;
mod extract;
mod gtfs;
mod parking;

/// Create a RawMap from OSM and other input data.
pub fn convert(
    osm_input_path: String,
    name: MapName,
    clip_path: Option<String>,
    opts: Options,
    timer: &mut Timer,
) -> RawMap {
    let mut map = RawMap::blank(name);
    // Do this early. Calculating Roads uses DrivingSide, for example!
    map.streets.config = opts.map_config.clone();

    if let Some(ref path) = clip_path {
        let pts = LonLat::read_osmosis_polygon(path).unwrap();
        let gps_bounds = GPSBounds::from(pts.clone());
        map.streets.boundary_polygon = Ring::must_new(gps_bounds.convert(&pts)).into_polygon();
        map.streets.gps_bounds = gps_bounds;
    }

    let (extract, amenity_points, bus_routes_on_roads) =
        extract::extract_osm(&mut map, &osm_input_path, clip_path, &opts, timer);
    map.bus_routes_on_roads = bus_routes_on_roads;
    let split_output = import_streets::split_ways::split_up_roads(&mut map.streets, extract, timer);
    clip_map(&mut map, timer);

    // Need to do a first pass of removing cul-de-sacs here, or we wind up with loop PolyLines when
    // doing the parking hint matching.
    map.streets.roads.retain(|r, _| r.i1 != r.i2);

    use_amenities(&mut map, amenity_points, timer);

    parking::apply_parking(&mut map, &opts, timer);

    import_streets::use_barrier_nodes(
        &mut map.streets,
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
        import_streets::filter_crosswalks(
            &mut map.streets,
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
        Road::new(
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

fn clip_map(map: &mut RawMap, timer: &mut Timer) {
    import_streets::clip::clip_map(&mut map.streets, timer).unwrap();

    let boundary_polygon = map.streets.boundary_polygon.clone();

    map.buildings.retain(|_, b| {
        b.polygon
            .points()
            .iter()
            .all(|pt| boundary_polygon.contains_pt(*pt))
    });

    let mut result_areas = Vec::new();
    for orig_area in map.areas.drain(..) {
        // If clipping fails, giving up on some areas is fine
        if let Ok(list) = map
            .streets
            .boundary_polygon
            .intersection(&orig_area.polygon)
        {
            for polygon in list {
                let mut area = orig_area.clone();
                area.polygon = polygon;
                result_areas.push(area);
            }
        }
    }
    map.areas = result_areas;

    // TODO Don't touch parking lots. It'll be visually obvious if a clip intersects one of these.
    // The boundary should be manually adjusted.
}

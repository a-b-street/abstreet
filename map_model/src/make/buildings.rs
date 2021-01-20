use std::collections::{BTreeMap, HashSet};

use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use abstutil::{Tags, Timer};
use geom::{Distance, HashablePt2D, Line, Polygon};

use crate::make::match_points_to_lanes;
use crate::raw::RawBuilding;
use crate::{
    osm, Amenity, Building, BuildingID, BuildingType, LaneID, Map, NamePerLanguage,
    OffstreetParking,
};

/// Finalize importing of buildings, mostly by matching them to the nearest sidewalk.
pub fn make_all_buildings(
    input: &BTreeMap<osm::OsmID, RawBuilding>,
    map: &Map,
    keep_bldg_tags: bool,
    timer: &mut Timer,
) -> Vec<Building> {
    timer.start("convert buildings");
    let mut center_per_bldg: BTreeMap<osm::OsmID, HashablePt2D> = BTreeMap::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    timer.start_iter("get building center points", input.len());
    for (id, b) in input {
        timer.next();
        let center = b.polygon.center().to_hashable();
        center_per_bldg.insert(*id, center);
        query.insert(center);
    }

    // equiv_pos could be a little closer, so use two buffers
    let sidewalk_buffer = Distance::meters(7.5);
    let sidewalk_pts = match_points_to_lanes(
        map.get_bounds(),
        query,
        map.all_lanes(),
        |l| l.is_walkable(),
        // Don't put connections too close to intersections
        sidewalk_buffer,
        // Try not to skip any buildings, but more than 1km from a sidewalk is a little much
        Distance::meters(1000.0),
        timer,
    );

    let mut results = Vec::new();
    timer.start_iter("match buildings to sidewalks", center_per_bldg.len());
    for (orig_id, bldg_center) in center_per_bldg {
        timer.next();
        if let Some(sidewalk_pos) = sidewalk_pts.get(&bldg_center) {
            let b = &input[&orig_id];
            let sidewalk_line = match Line::new(bldg_center.to_pt2d(), sidewalk_pos.pt(map)) {
                Some(l) => trim_path(&b.polygon, l),
                None => {
                    timer.warn(format!(
                        "Skipping building {} because front path has 0 length",
                        orig_id
                    ));
                    continue;
                }
            };

            let id = BuildingID(results.len());

            let mut rng = XorShiftRng::seed_from_u64(orig_id.inner() as u64);
            // TODO is it worth using height or building:height as an alternative if not tagged?
            let levels = b
                .osm_tags
                .get("building:levels")
                .and_then(|x| x.parse::<f64>().ok())
                .unwrap_or(1.0);

            results.push(Building {
                id,
                polygon: b.polygon.clone(),
                levels,
                address: get_address(&b.osm_tags, sidewalk_pos.lane(), map),
                name: NamePerLanguage::new(&b.osm_tags),
                orig_id,
                label_center: b.polygon.polylabel(),
                amenities: if keep_bldg_tags {
                    b.amenities.clone()
                } else {
                    b.amenities
                        .iter()
                        .map(|a| {
                            let mut a = a.clone();
                            a.osm_tags = Tags::new(BTreeMap::new());
                            a
                        })
                        .collect()
                },
                bldg_type: classify_bldg(
                    &b.osm_tags,
                    &b.amenities,
                    levels,
                    b.polygon.area(),
                    &mut rng,
                ),
                parking: if let Some(n) = b.public_garage_name.clone() {
                    OffstreetParking::PublicGarage(n, b.num_parking_spots)
                } else {
                    OffstreetParking::Private(
                        b.num_parking_spots,
                        b.osm_tags.is("building", "parking") || b.osm_tags.is("amenity", "parking"),
                    )
                },
                osm_tags: if keep_bldg_tags {
                    b.osm_tags.clone()
                } else {
                    Tags::new(BTreeMap::new())
                },

                sidewalk_pos: *sidewalk_pos,
                driveway_geom: sidewalk_line.to_polyline(),
            });
        }
    }

    timer.note(format!(
        "Discarded {} buildings that weren't close enough to a sidewalk",
        input.len() - results.len()
    ));
    timer.stop("convert buildings");

    results
}

// Adjust the path to start on the building's border, not center
fn trim_path(poly: &Polygon, path: Line) -> Line {
    for bldg_line in poly.points().windows(2) {
        if let Some(l1) = Line::new(bldg_line[0], bldg_line[1]) {
            if let Some(hit) = l1.intersection(&path) {
                if let Some(l2) = Line::new(hit, path.pt2()) {
                    return l2;
                }
            }
        }
    }
    // Just give up
    path
}

fn get_address(tags: &Tags, sidewalk: LaneID, map: &Map) -> String {
    match (tags.get("addr:housenumber"), tags.get("addr:street")) {
        (Some(num), Some(st)) => format!("{} {}", num, st),
        (None, Some(st)) => format!("??? {}", st),
        _ => format!("??? {}", map.get_parent(sidewalk).get_name(None)),
    }
}

fn classify_bldg(
    tags: &Tags,
    amenities: &Vec<Amenity>,
    levels: f64,
    ground_area_sq_meters: f64,
    rng: &mut XorShiftRng,
) -> BuildingType {
    // used: top values from https://taginfo.openstreetmap.org/keys/building#values (>100k uses)

    let mut commercial = false;

    let area_sq_meters = levels * ground_area_sq_meters;

    // These are produced by get_bldg_amenities in convert_osm/src/osm_reader.rs.
    // TODO: is it safe to assume all amenities are commercial?
    // TODO: consider converting amenities to an enum - maybe with a catchall case for the long
    //       tail of rarely used enums.
    if !amenities.is_empty() {
        commercial = true;
    }

    if tags.is("ruins", "yes") {
        if commercial {
            return BuildingType::Commercial(0);
        }
        return BuildingType::Empty;
    }

    let mut residents: usize = 0;
    let mut workers: usize = 0;

    if tags.is_any(
        "building",
        vec![
            "office",
            "industrial",
            "commercial",
            "retail",
            "warehouse",
            "civic",
            "public",
        ],
    ) {
        // 1 person per 10 square meters
        // TODO: Hone in this parameter. Space per person varies with (among other things):
        //  - building type. e.g. office vs. warehouse
        //  - regional/cultural norms
        workers = (area_sq_meters / 10.0) as usize;
    } else if tags.is_any(
        "building",
        vec!["school", "university", "construction", "church"],
    ) {
        // TODO: special handling in future
        return BuildingType::Empty;
    } else if tags.is_any(
        "building",
        vec![
            "garage",
            "garages",
            "shed",
            "roof",
            "greenhouse",
            "farm_auxiliary",
            "barn",
            "service",
        ],
    ) {
        return BuildingType::Empty;
    } else if tags.is_any(
        "building",
        vec!["house", "detached", "semidetached_house", "farm"],
    ) {
        residents = rng.gen_range(0..3);
    } else if tags.is_any("building", vec!["hut", "static_caravan", "cabin"]) {
        residents = rng.gen_range(0..2);
    } else if tags.is_any("building", vec!["apartments", "terrace", "residential"]) {
        // 1 person per 10 square meters
        // TODO: Hone in this parameter. Space per person varies with (among other things):
        //  - building type. e.g. apartment vs single family
        //  - regional/cultural norms
        residents = (area_sq_meters / 10.0) as usize;
    } else {
        residents = rng.gen_range(0..2);
    }

    if commercial && workers == 0 {
        // TODO: Come up with a better measure
        workers = (residents as f64 / 3.0) as usize;
    }

    if commercial {
        if residents > 0 {
            return BuildingType::ResidentialCommercial(residents, workers);
        }
        return BuildingType::Commercial(workers);
    }
    return BuildingType::Residential {
        num_residents: residents,
        num_housing_units: 1,
    };
}

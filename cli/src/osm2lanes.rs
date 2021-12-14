use std::collections::BTreeMap;

use serde::Serialize;

use abstutil::Timer;
use map_model::{osm, Direction, DrivingSide, LaneType, Map, Road};

/// Given a map, print a JSON array of test cases for osm2lanes.
pub fn run(map_path: String) {
    let map = Map::load_synchronously(map_path, &mut Timer::throwaway());
    let driving_side = match map.get_config().driving_side {
        DrivingSide::Right => "right",
        DrivingSide::Left => "left",
    };

    let mut tests = Vec::new();
    for road in map.all_roads() {
        if let Some(tc) = transform(road, driving_side) {
            tests.push(tc);
        }
    }
    println!("{}", abstutil::to_json(&tests));
}

/// This matches the test case format used by https://github.com/a-b-street/osm2lanes.
///
/// Note this internally uses strings instead of enums; the purpose is just to serialize JSON right
/// now. When we move the Rust implementation into the osm2lanes repository, at that point we'll
/// use better types from there directly.
#[derive(Serialize)]
struct TestCase {
    way: String,
    tags: BTreeMap<String, String>,
    driving_side: String,
    output: Vec<LaneSpec>,
}

#[derive(Serialize)]
struct LaneSpec {
    #[serde(rename = "type")]
    lane_type: String,
    direction: String,
}

fn transform(road: &Road, driving_side: &str) -> Option<TestCase> {
    let mut result = TestCase {
        way: road.orig_id.osm_way_id.to_string(),
        tags: strip_tags(road.osm_tags.clone().into_inner()),
        driving_side: driving_side.to_string(),
        output: Vec::new(),
    };
    for lane in &road.lanes {
        result.output.push(LaneSpec {
            lane_type: match lane.lane_type {
                LaneType::Driving => "driveway",
                LaneType::Parking => "parking_lane",
                LaneType::Sidewalk => "sidewalk",
                LaneType::Shoulder => "shoulder",
                LaneType::Biking => "cycleway",
                // Until we decide on the schema for some of these other lane types, don't generate
                // test cases for any roads wth them
                LaneType::Bus => {
                    return None;
                }
                LaneType::SharedLeftTurn => "shared_left_turn",
                LaneType::Construction => {
                    return None;
                }
                LaneType::LightRail => {
                    return None;
                }
                LaneType::Buffer(_) => {
                    return None;
                }
                LaneType::Footway => {
                    return None;
                }
            }
            .to_string(),
            direction: match lane.dir {
                Direction::Fwd => "forward",
                Direction::Back => "backward",
            }
            .to_string(),
        });
    }
    Some(result)
}

fn strip_tags(mut tags: BTreeMap<String, String>) -> BTreeMap<String, String> {
    // These aren't OSM tags; they're just added by A/B Street for internal use
    tags.remove(osm::OSM_WAY_ID);
    tags.remove(osm::ENDPT_FWD);
    tags.remove(osm::ENDPT_BACK);
    // If these are present, the parking and sidewalk tags may have been inserted by A/B Street.
    // The test case's tags may not match what's really in OSM, but the OSM tags will still follow
    // the normal OSM schema.
    tags.remove(osm::INFERRED_PARKING);
    tags.remove(osm::INFERRED_SIDEWALKS);

    // These're common tags that just add noise to test cases. Should we instead explicitly list
    // tags used by the implementation?
    tags.remove("maxspeed");
    tags.remove("name");
    tags.remove("old_ref");
    tags.remove("ref");

    tags
}

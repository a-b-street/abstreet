use abstutil::Tags;
use map_model::{Direction, EditRoad, LaneSpec, LaneType};

/// Returns the index where the new lane was inserted
pub fn add_new_lane(road: &mut EditRoad, lt: LaneType, osm_tags: &Tags) -> usize {
    let dir = match lt {
        LaneType::Driving => determine_lane_dir(road, lt, true),
        LaneType::Biking | LaneType::Bus | LaneType::Parking | LaneType::Construction => {
            let relevant_lanes: Vec<&LaneSpec> =
                road.lanes_ltr.iter().filter(|x| x.lt == lt).collect();
            if !relevant_lanes.is_empty() {
                // When a lane already exists then default to the direction on the other side of the
                // road
                if relevant_lanes[0].dir == Direction::Fwd {
                    Direction::Back
                } else {
                    Direction::Fwd
                }
            } else {
                // If no lanes exist then default to the majority direction to help deal with one
                // way streets, etc.
                determine_lane_dir(road, lt, false)
            }
        }
        LaneType::Sidewalk => {
            if !road.lanes_ltr[0].lt.is_walkable() {
                road.lanes_ltr[0].dir
            } else {
                road.lanes_ltr.last().unwrap().dir
            }
        }
        LaneType::Buffer(_) => {
            // TODO Look for the bike lane that's missing a buffer
            Direction::Fwd
        }
        _ => unreachable!(),
    };

    let idx = match lt {
        // In the middle (where the direction changes)
        LaneType::Driving => road
            .lanes_ltr
            .windows(2)
            .position(|pair| pair[0].dir != pair[1].dir)
            .map(|x| x + 1)
            .unwrap_or(road.lanes_ltr.len()),
        // Place on the dir side, before any sidewalk
        LaneType::Biking | LaneType::Bus | LaneType::Parking | LaneType::Construction => {
            default_outside_lane_placement(road, dir)
        }
        // Place it where it's missing
        LaneType::Sidewalk => {
            if !road.lanes_ltr[0].lt.is_walkable() {
                0
            } else {
                road.lanes_ltr.len()
            }
        }
        LaneType::Buffer(_) => {
            // TODO Look for the bike lane that's missing a buffer
            0
        }
        _ => unreachable!(),
    };

    road.lanes_ltr.insert(
        idx,
        LaneSpec {
            lt,
            dir,
            width: LaneSpec::typical_lane_widths(lt, osm_tags)[0].0,
        },
    );
    idx
}

/// Place the new lane according to its direction on the outside unless the outside is walkable in
/// which case place inside the walkable lane
fn default_outside_lane_placement(road: &mut EditRoad, dir: Direction) -> usize {
    if road.lanes_ltr[0].dir == dir {
        if road.lanes_ltr[0].lt.is_walkable() {
            1
        } else {
            0
        }
    } else if road.lanes_ltr.last().unwrap().lt.is_walkable() {
        road.lanes_ltr.len() - 1
    } else {
        road.lanes_ltr.len()
    }
}

/// If there are more lanes of type lt pointing forward, then insert the new one backwards, and
/// vice versa
fn determine_lane_dir(road: &mut EditRoad, lt: LaneType, minority: bool) -> Direction {
    if (road
        .lanes_ltr
        .iter()
        .filter(|x| x.dir == Direction::Fwd && x.lt == lt)
        .count() as f64
        / road.lanes_ltr.iter().filter(|x| x.lt == lt).count() as f64)
        <= 0.5
    {
        if minority {
            Direction::Fwd
        } else {
            Direction::Back
        }
    } else if minority {
        Direction::Back
    } else {
        Direction::Fwd
    }
}

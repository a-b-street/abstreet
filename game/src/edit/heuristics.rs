use abstutil::Tags;
use map_model::{Direction, EditRoad, LaneSpec, LaneType};

/// Returns the index where the new lane was inserted
pub fn add_new_lane(road: &mut EditRoad, lt: LaneType, osm_tags: &Tags) -> usize {
    let (dir, idx);
    match lt {
        LaneType::Driving => {
            dir = determine_lane_dir(road, lt, true);
            // In the middle (where the direction changes)
            idx = road
                .lanes_ltr
                .windows(2)
                .position(|pair| pair[0].dir != pair[1].dir)
                .map(|x| x + 1)
                .unwrap_or(road.lanes_ltr.len());
        }
        LaneType::Biking | LaneType::Bus | LaneType::Parking | LaneType::Construction => {
            let relevant_lanes: Vec<&LaneSpec> =
                road.lanes_ltr.iter().filter(|x| x.lt == lt).collect();
            dir = if !relevant_lanes.is_empty() {
                // When a lane already exists, then default to the direction on the other side of
                // the road
                relevant_lanes[0].dir.opposite()
            } else {
                // If no lanes exist, then default to the majority direction, to help deal with
                // one-way streets
                determine_lane_dir(road, lt, false)
            };

            // Place on the dir side, before any sidewalk
            idx = default_outside_lane_placement(road, dir);
        }
        LaneType::Sidewalk => {
            // Place where it's missing
            if !road.lanes_ltr[0].lt.is_walkable() {
                dir = road.lanes_ltr[0].dir;
                idx = 0;
            } else {
                dir = road.lanes_ltr.last().unwrap().dir;
                idx = road.lanes_ltr.len();
            }
        }
        LaneType::Buffer(_) => {
            // TODO Look for the bike lane that's missing a buffer
            dir = Direction::Fwd;
            idx = 0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_new_lane() {
        let mut ok = true;
        for (description, input_lt, input_dir, new_lt, expected_lt, expected_dir) in vec![
            (
                "Two-way with parking, adding bike lane to first side",
                "spddps",
                "vvv^^^",
                LaneType::Biking,
                // TODO Current heuristics put it between parking and sidewalk, but this isn't
                // right
                "spddpbs",
                "vvv^^^^",
            ),
            (
                "Two-way with parking, adding bike lane to second side",
                "spddpbs",
                "vvv^^^^",
                LaneType::Biking,
                // TODO Current heuristics put it between parking and sidewalk, but this isn't
                // right
                "sbpddpbs",
                "vvvv^^^^",
            ),
            (
                "Add driving lane, balanced numbers",
                "sdds",
                "vv^^",
                LaneType::Driving,
                "sddds",
                "vv^^^",
            ),
            (
                "Add driving lane, imbalanced",
                "sddds",
                "vv^^^",
                LaneType::Driving,
                "sdddds",
                "vvv^^^",
            ),
        ] {
            let input = EditRoad::create_for_test(input_lt, input_dir);
            let mut actual_output = input.clone();
            add_new_lane(&mut actual_output, new_lt, &Tags::empty());
            actual_output.check_lanes_ltr(
                description.to_string(),
                input_lt,
                input_dir,
                expected_lt,
                expected_dir,
                &mut ok,
            );
        }
        assert!(ok);
    }
}

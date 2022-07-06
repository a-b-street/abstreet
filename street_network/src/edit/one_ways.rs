use geom::Distance;

use crate::{Direction, DrivingSide, LaneSpec, LaneType};

impl LaneSpec {
    /// Convert the driving lanes of a road between one-way forwards, one-way backwards, and
    /// bidirectional. It should be idempotent to apply this 3 times in a row. Even when an
    /// existing one-way street is narrow, it can be made bidirectional by splitting into two very
    /// narrow lanes.
    pub fn toggle_road_direction(lanes_ltr: &mut Vec<LaneSpec>, driving_side: DrivingSide) {
        let leftmost_dir = if driving_side == DrivingSide::Right {
            Direction::Back
        } else {
            Direction::Fwd
        };
        let oneway_dir = LaneSpec::oneway_for_driving(lanes_ltr);
        let mut num_driving_lanes = lanes_ltr
            .iter()
            .filter(|lane| lane.lt == LaneType::Driving)
            .count();

        // Pre-processing: if it's one-way backwards and there's only one driving lane,
        // split it into two narrow pieces
        if oneway_dir == Some(Direction::Back) && num_driving_lanes == 1 {
            // TODO If there's parking, use that
            let idx = lanes_ltr
                .iter()
                .position(|x| x.lt == LaneType::Driving)
                .unwrap();
            lanes_ltr[idx].width *= 0.5;
            lanes_ltr.insert(idx, lanes_ltr[idx].clone());
            num_driving_lanes = 2;
        }
        // And undo the above
        if oneway_dir == None && num_driving_lanes == 2 {
            let idx = lanes_ltr
                .iter()
                .position(|x| x.lt == LaneType::Driving)
                .unwrap();
            // Is it super narrow?
            // TODO Potentially brittle. SERVICE_ROAD_LANE_THICKNESS is 1.5,
            // NORMAL_LANE_THICKNESS is 2.5. Half of either one is less than 1.5.
            if lanes_ltr[idx].width < Distance::meters(1.5) {
                lanes_ltr.remove(idx);
                lanes_ltr[idx].width *= 2.0;
            }
        }

        let mut driving_lanes_so_far = 0;
        for lane in lanes_ltr {
            if lane.lt == LaneType::Driving {
                driving_lanes_so_far += 1;
                match oneway_dir {
                    Some(Direction::Fwd) => {
                        // If it's one-way forwards, flip the direction
                        lane.dir = Direction::Back;
                    }
                    Some(Direction::Back) => {
                        // If it's one-way backwards, make it bidirectional. Split the
                        // directions down the middle
                        if (driving_lanes_so_far as f64) / (num_driving_lanes as f64) <= 0.5 {
                            lane.dir = leftmost_dir;
                        } else {
                            lane.dir = leftmost_dir.opposite();
                        }
                    }
                    None => {
                        // TODO If it's narrow...
                        // If it's bidirectional, make it one-way
                        lane.dir = Direction::Fwd;
                    }
                }
            }
        }
    }
}

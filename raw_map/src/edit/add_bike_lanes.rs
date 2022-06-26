use abstutil::Tags;

use crate::{BufferType, Direction, DrivingSide, LaneSpec, LaneType};

impl LaneSpec {
    pub fn maybe_add_bike_lanes(
        lanes_ltr: &mut Vec<LaneSpec>,
        buffer_type: Option<BufferType>,
        driving_side: DrivingSide,
    ) {
        let dummy_tags = Tags::empty();

        // First decompose the existing lanes back into a fwd_side and back_side. This is not quite the
        // inverse of assemble_ltr -- lanes on the OUTERMOST side of the road are first.
        let mut fwd_side = Vec::new();
        let mut back_side = Vec::new();
        for spec in lanes_ltr.drain(..) {
            if spec.dir == Direction::Fwd {
                fwd_side.push(spec);
            } else {
                back_side.push(spec);
            }
        }
        if driving_side == DrivingSide::Right {
            fwd_side.reverse();
        } else {
            back_side.reverse();
        }

        for (dir, side) in [
            (Direction::Fwd, &mut fwd_side),
            (Direction::Back, &mut back_side),
        ] {
            // For each side, start searching outer->inner. If there's parking, replace it. If there's
            // multiple driving lanes, fallback to changing the rightmost. If there's a bus lane, put
            // the bike lanes on the outside of it.
            let mut parking_lane = None;
            let mut first_driving_lane = None;
            let mut bus_lane = None;
            let mut num_driving_lanes = 0;
            let mut already_has_bike_lane = false;
            for (idx, spec) in side.iter().enumerate() {
                if spec.lt == LaneType::Parking && parking_lane.is_none() {
                    parking_lane = Some(idx);
                }
                if spec.lt == LaneType::Driving && first_driving_lane.is_none() {
                    first_driving_lane = Some(idx);
                }
                if spec.lt == LaneType::Driving {
                    num_driving_lanes += 1;
                }
                if spec.lt == LaneType::Bus && bus_lane.is_none() {
                    bus_lane = Some(idx);
                }
                if spec.lt == LaneType::Biking {
                    already_has_bike_lane = true;
                }
            }
            if already_has_bike_lane {
                // TODO If it's missing a buffer and one is requested, fill it in
                continue;
            }
            // So if a road is one-way, this shouldn't add a bike lane to the off-side.
            let idx = if let Some(idx) = parking_lane {
                if num_driving_lanes == 0 {
                    None
                } else {
                    Some(idx)
                }
            } else if bus_lane.is_some() && num_driving_lanes > 1 {
                // Nuke the driving lane
                side.remove(first_driving_lane.unwrap());
                // Copy the bus lane (because the code below always overwrites idx)
                let bus_idx = bus_lane.unwrap();
                side.insert(bus_idx, side[bus_idx].clone());
                // Then put the bike lane on the outside of the bus lane
                Some(bus_idx)
            } else if num_driving_lanes > 1 {
                first_driving_lane
            } else {
                None
            };
            if let Some(idx) = idx {
                side[idx] = LaneSpec {
                    lt: LaneType::Biking,
                    dir,
                    width: LaneSpec::typical_lane_widths(LaneType::Biking, &dummy_tags)[0].0,
                };
                if let Some(buffer) = buffer_type {
                    side.insert(
                        idx + 1,
                        LaneSpec {
                            lt: LaneType::Buffer(buffer),
                            dir,
                            width: LaneSpec::typical_lane_widths(
                                LaneType::Buffer(buffer),
                                &dummy_tags,
                            )[0]
                            .0,
                        },
                    );
                }
            }
        }

        // Now re-assemble...
        if driving_side == DrivingSide::Right {
            *lanes_ltr = back_side;
            fwd_side.reverse();
            lanes_ltr.extend(fwd_side);
        } else {
            *lanes_ltr = fwd_side;
            back_side.reverse();
            lanes_ltr.extend(back_side);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maybe_add_bike_lanes() {
        let with_buffers = true;
        let no_buffers = false;

        let mut ok = true;
        for (
            description,
            url,
            driving_side,
            input_lt,
            input_dir,
            buffer,
            expected_lt,
            expected_dir,
        ) in vec![
            (
                "Two-way without room",
                "https://www.openstreetmap.org/way/537698750",
                DrivingSide::Right,
                "sdds",
                "vv^^",
                no_buffers,
                "sdds",
                "vv^^",
            ),
            (
                "Two-way with parking, adding buffers",
                "https://www.openstreetmap.org/way/40790122",
                DrivingSide::Right,
                "spddps",
                "vvv^^^",
                with_buffers,
                "sb|dd|bs",
                "vvvv^^^^",
            ),
            (
                "Two-way with parking, no buffers",
                "https://www.openstreetmap.org/way/40790122",
                DrivingSide::Right,
                "spddps",
                "vvv^^^",
                no_buffers,
                "sbddbs",
                "vvv^^^",
            ),
            (
                "Two-way without parking but many lanes",
                "https://www.openstreetmap.org/way/394737309",
                DrivingSide::Right,
                "sddddds",
                "vvv^^^^",
                with_buffers,
                "sb|ddd|bs",
                "vvvv^^^^^",
            ),
            (
                "One-way with parking on both sides",
                "https://www.openstreetmap.org/way/559660378",
                DrivingSide::Right,
                "spddps",
                "vv^^^^",
                with_buffers,
                "spdd|bs",
                "vv^^^^^",
            ),
            (
                "One-way with bus lanes",
                "https://www.openstreetmap.org/way/52840106",
                DrivingSide::Right,
                "ddBs",
                "^^^^",
                with_buffers,
                "dB|bs",
                "^^^^^",
            ),
            (
                "Two-way with bus lanes",
                "https://www.openstreetmap.org/way/368670632",
                DrivingSide::Right,
                "sBddCddBs",
                "vvvv^^^^^",
                with_buffers,
                "sb|BdCdB|bs",
                "vvvvv^^^^^^",
            ),
            (
                "Two-way without room, on a left-handed map",
                "https://www.openstreetmap.org/way/436838877",
                DrivingSide::Left,
                "sdds",
                "^^vv",
                no_buffers,
                "sdds",
                "^^vv",
            ),
            (
                "Two-way, on a left-handed map",
                "https://www.openstreetmap.org/way/312457180",
                DrivingSide::Left,
                "sdddds",
                "^^^vvv",
                no_buffers,
                "sbddbs",
                "^^^vvv",
            ),
            (
                "One side already has a bike lane",
                "https://www.openstreetmap.org/way/427757048",
                DrivingSide::Right,
                "spbddps",
                "vvvv^^^",
                with_buffers,
                "spbdd|bs",
                "vvvv^^^^",
            ),
        ] {
            let input = LaneSpec::create_for_test(input_lt, input_dir);
            let mut actual_output = input.clone();
            LaneSpec::maybe_add_bike_lanes(
                &mut actual_output,
                if buffer {
                    Some(BufferType::FlexPosts)
                } else {
                    None
                },
                driving_side,
            );
            LaneSpec::check_lanes_ltr(
                &actual_output,
                format!("{} (example from {})", description, url),
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

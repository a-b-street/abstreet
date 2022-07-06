// It's nice to group complex transformations of lanes here -- they can potentially share logic or
// testing infrastructure.

mod add_bike_lanes;
mod add_new_lane;
mod one_ways;

use geom::Distance;

use crate::{Direction, LaneSpec, LaneType};

impl LaneSpec {
    /// Transforms a string describing lane types and directions, like "spddps" and "vv^^^^^", into
    /// a list of LaneSpecs. Useful for unit tests.
    pub fn create_for_test(input_lt: &str, input_dir: &str) -> Vec<LaneSpec> {
        assert_eq!(input_lt.len(), input_dir.len());
        input_lt
            .chars()
            .zip(input_dir.chars())
            .map(|(lt, dir)| LaneSpec {
                lt: LaneType::from_char(lt),
                dir: if dir == '^' {
                    Direction::Fwd
                } else {
                    Direction::Back
                },
                // Dummy
                width: Distance::ZERO,
            })
            .collect()
    }

    /// This is meant for table-driven unit tests. Call this on the transformed / output lanes. If
    /// the lanes don't match, `ok` will be set to false and appropriate errors will be printed.
    pub fn check_lanes_ltr(
        actual_lanes_ltr: &[LaneSpec],
        description: String,
        input_lt: &str,
        input_dir: &str,
        expected_lt: &str,
        expected_dir: &str,
        ok: &mut bool,
    ) {
        let actual_lt: String = actual_lanes_ltr.iter().map(|s| s.lt.to_char()).collect();
        let actual_dir: String = actual_lanes_ltr
            .iter()
            .map(|s| if s.dir == Direction::Fwd { '^' } else { 'v' })
            .collect();

        if actual_lt != expected_lt || actual_dir != expected_dir {
            *ok = false;
            println!("{}", description);
            println!("Input:");
            println!("    {}", input_lt);
            println!("    {}", input_dir);
            println!("Got:");
            println!("    {}", actual_lt);
            println!("    {}", actual_dir);
            println!("Expected:");
            println!("    {}", expected_lt);
            println!("    {}", expected_dir);
            println!();
        }
    }
}

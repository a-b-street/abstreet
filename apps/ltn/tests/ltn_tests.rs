#[cfg(test)]
mod tests {
    use tests::get_test_file_path;

    #[test]
    /// Tests that the `get_test_file_path` convenience function itself works as expected.
    /// Note that this tests is identical to `test_get_test_file_path_tests_crate`. This is
    /// to assert that the behaviour of `get_test_file_path` is identical from different
    /// locations within the workspace.
    fn test_get_test_file_path_ltn_crate() -> Result<(), anyhow::Error> {
        let sample_test_files = vec![
            "input/divided_highway_split.osm",
            "input/left_turn_and_bike_lane.osm",
            "input/multiple_left_turn_lanes.osm",
            "input/false_positive_u_turns.osm",
            "input/turn_restriction_ltn_boundary.osm",
            "goldenfiles/turn_types/divided_highway_split.txt",
            "goldenfiles/turn_types/left_turn_and_bike_lane.txt",
            "goldenfiles/turn_types/multiple_left_turn_lanes.txt",
            "goldenfiles/turn_types/false_positive_u_turns.txt",
            "goldenfiles/turn_types/turn_restriction_ltn_boundary.txt",
        ];
        
        // test that each of the sample test files can be located
        assert!(sample_test_files.iter().all(|f| get_test_file_path(String::from(*f)).is_ok()));

        let sample_test_files = vec![
            "does_not_exist",
            "/really/shoud/not/exist",
        ];
        
        // test that each of the sample test files cannot be located
        assert!(sample_test_files.iter().all(|f| get_test_file_path(String::from(*f)).is_err()));

        Ok(())
    }
}
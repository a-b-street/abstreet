#[cfg(test)]
mod tests {
    use abstutil::Timer;
    use fs_err::File;
    use geom::Pt2D;
    use ltn::logic::turn_restrictions::FocusedTurns;
    use ltn::pages::design_ltn::turn_restrictions::handle_edited_turn_restrictions;
    use ltn::save::Proposals;
    use map_model::RoadID;
    use std::io::Write;
    use tests::{compare_with_goldenfile, get_test_file_path, import_map};

    #[test]
    /// Tests that the `get_test_file_path` convenience function itself works as expected.
    /// Note that this tests is identical to `test_get_test_file_path_tests_crate`. This is
    /// to assert that the behaviour of `get_test_file_path` is identical from different
    /// locations within the workspace.
    fn test_get_test_file_path_ltn_crate() {
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
        assert!(sample_test_files
            .iter()
            .all(|f| get_test_file_path(String::from(*f)).is_ok()));

        let sample_test_files = vec!["does_not_exist", "/really/should/not/exist"];

        // test that each of the sample test files cannot be located
        assert!(sample_test_files
            .iter()
            .all(|f| get_test_file_path(String::from(*f)).is_err()));
    }

    /// Integration test for turn restrictions edits
    /// - loads a map
    /// - make some edits
    /// - save
    /// - assert that the edits are correctly represented in the saved file
    #[test]
    fn test_edit_turn_restrictions() {
        // Load a sample map
        let input_file =
            get_test_file_path(String::from("input/turn_restriction_ltn_boundary.osm"));
        let mut map = import_map(input_file.unwrap());
        let mut proposals = Proposals::new(&map, &mut Timer::throwaway());

        // make some edits to the turn restrictions
        let r = RoadID(11);
        // south west (Remove existing turn restriction)
        let click_pt_1 = Pt2D::new(192.5633, 215.7847);
        let target_road_1 = RoadID(4);
        // north east (Add a new turn restriction)
        let click_pt_2 = Pt2D::new(214.7931, 201.7212);
        let target_road_2 = RoadID(12);

        for (click_pt, target_r) in [(click_pt_1, target_road_1), (click_pt_2, target_road_2)] {
            let ft = FocusedTurns::new(r, click_pt, &map);

            let mut edits = map.get_edits().clone();
            let erc = map.edit_road_cmd(ft.from_r, |new| {
                handle_edited_turn_restrictions(new, &ft, target_r)
            });
            println!("erc={:?}", erc);
            edits.commands.push(erc);

            // manually sync map and proposals (normally done by `app.apply_edits()`)
            proposals.before_edit(edits.clone());
            map.must_apply_edits(edits, &mut Timer::throwaway());
        }

        // save the map
        let p = proposals.get_current();

        // Get edit commands in json form (ignoring partitioning)
        let actual = serde_json::to_string_pretty(&p.edits.to_permanent(&map)).unwrap();

        // update goldenfile if required
        let dump_turn_goldenfile = false;
        let goldenfile_path = get_test_file_path(String::from(
            "goldenfiles/ltn_proposals/edit_turn_restrictions.json",
        ))
        .unwrap();

        if dump_turn_goldenfile {
            let mut f_types = File::create(&goldenfile_path).unwrap();
            writeln!(f_types, "{}", actual).unwrap();

            // panic so that we done get into the habit of re-writing the goldenfile
            panic!("Automatically fail when the goldenfiles are regenerated. This is so the test is not accidentally left in a set where there goldenfiles are recreated on each run, and the test does not achieve its purpose.");
        }

        // finally compare with goldenfile
        assert!(compare_with_goldenfile(actual, goldenfile_path).unwrap());
    }
}

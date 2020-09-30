use crate::import_map;
use map_model::{LaneID, TurnType};

// If this test gets fully fleshed out, there would be many more of these. This one means the
// southern road, inbound to the intersection, the left lane of it.
const S_IN_LEFT: LaneID = LaneID(2);

#[test]
fn test_left_turn_lane() {
    let map = import_map(four_way());

    // Assert the hardcoded ID is reasonable
    assert_eq!("south", map.get_parent(S_IN_LEFT).get_name(None));
    assert!(map.get_l(S_IN_LEFT).is_driving());

    // TODO This is quite a weak assertion. I want to express that there's only the left turn from
    // this lane, and S_IN_RIGHT has the two straight turns and the right turn. But it'll be so
    // verbose.
    assert_eq!(
        TurnType::Left,
        map.get_turns_from_lane(S_IN_LEFT)[0].turn_type
    );
}

// A map with 4 roads (north, south, east, west) and one intersection. The north/south roads have 4
// lanes, the east/west just 2. The south road has a left turn lane.
fn four_way() -> String {
    format!(
        r#"<?xml version='1.0' encoding='UTF-8'?><osm>
        <bounds minlon="0.0" maxlon="0.01" minlat="0.0" maxlat="0.01"/>
        <node id="1" lon="0.005" lat="0.005"/>
        <node id="2" lon="0.005" lat="-1.0"/>
        <node id="3" lon="0.005" lat="1.0"/>
        <node id="4" lon="-0.1" lat="0.005"/>
        <node id="5" lon="1.0" lat="0.005"/>
        <way id="100">
            <nd ref="1"/>
            <nd ref="2"/>
            <tag k="name" v="south"/>
            <tag k="highway" v="primary"/>
            <tag k="lanes" v="4"/>
            <tag k="sidewalk" v="both"/>
            <tag k="turn:lanes:backward" v="left|"/>
        </way>
        <way id="101">
            <nd ref="1"/>
            <nd ref="3"/>
            <tag k="name" v="north"/>
            <tag k="highway" v="primary"/>
            <tag k="lanes" v="4"/>
            <tag k="sidewalk" v="both"/>
        </way>
        <way id="102">
            <nd ref="1"/>
            <nd ref="4"/>
            <tag k="name" v="west"/>
            <tag k="highway" v="residential"/>
            <tag k="lanes" v="2"/>
            <tag k="sidewalk" v="both"/>
        </way>
        <way id="103">
            <nd ref="1"/>
            <nd ref="5"/>
            <tag k="name" v="east"/>
            <tag k="highway" v="residential"/>
            <tag k="lanes" v="2"/>
            <tag k="sidewalk" v="both"/>
        </way>
    </osm>"#
    )
}

use map_model::{Direction, Lane, LaneType, Map, RoadID};

/// Exports a single road to Streetmix's format, returns the filename
pub fn export(r: RoadID, map: &Map) -> String {
    let path = format!("streetmix_export_{}.json", r.0);
    let street = road(r, map);
    abstutil::write_json(path.clone(), &street);
    path
}

fn road(id: RoadID, map: &Map) -> serde_json::Map<String, serde_json::value::Value> {
    let r = map.get_r(id);
    let mut street = serde_json::Map::new();
    street.insert("schemaVersion".to_string(), 24.into());
    // TODO Many more fields

    let mut segments = Vec::new();
    for (l, dir, _) in r.lanes_ltr() {
        segments.push(serde_json::value::Value::Object(lane(map.get_l(l), dir)));
    }
    street.insert(
        "segments".to_string(),
        serde_json::value::Value::Array(segments),
    );

    street
}

fn lane(lane: &Lane, dir: Direction) -> serde_json::Map<String, serde_json::value::Value> {
    let mut segment = serde_json::Map::new();
    segment.insert("id".to_string(), lane.id.to_string().into());
    segment.insert("width".to_string(), lane.width.to_feet().into());

    // TODO I'm taking wild stabs at these values for now. Once I can visualize the results, will
    // iterate on these.
    let (segment_type, variant) = match lane.lane_type {
        LaneType::Driving => match dir {
            Direction::Fwd => ("drive-lane", "inbound|car"),
            Direction::Back => ("drive-lane", "outbound|car"),
        },
        LaneType::Parking => match dir {
            Direction::Fwd => ("parking-lane", "inbound|left"),
            Direction::Back => ("parking-lane", "outbound|right"),
        },
        LaneType::Sidewalk => ("sidewalk", "dense"),
        LaneType::Shoulder => ("sidewalk", "dense"),
        LaneType::Biking => match dir {
            Direction::Fwd => ("bike-lane", "inbound|green|road"),
            Direction::Back => ("bike-lane", "outbound|green|road"),
        },
        LaneType::Bus => match dir {
            Direction::Fwd => ("bus-lane", "inbound|shared"),
            Direction::Back => ("bus-lane", "outbound|shared"),
        },
        LaneType::SharedLeftTurn => ("TODO", "TODO"),
        LaneType::Construction => ("TODO", "TODO"),
        LaneType::LightRail => ("TODO", "TODO"),
    };
    segment.insert("type".to_string(), segment_type.into());
    segment.insert("variant".to_string(), variant.into());

    segment
}

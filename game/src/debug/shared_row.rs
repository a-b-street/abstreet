use geojson::feature::Id;
use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
use map_model::{Lane, LaneType, Map, RoadID};

// Exports to https://github.com/d-wasserman/shared-row/
pub fn export(roads: Vec<RoadID>, map: &Map) {
    let geojson = GeoJson::from(FeatureCollection {
        bbox: None,
        features: roads.into_iter().map(|r| road(r, map)).collect(),
        foreign_members: None,
    });
    abstutil::write_json("shared_row_export.json".to_string(), &geojson);
}

fn road(id: RoadID, map: &Map) -> Feature {
    let r = map.get_r(id);
    let mut properties = serde_json::Map::new();
    // TODO Generate https://github.com/sharedstreets/sharedstreets-ref-system IDs
    properties.insert("OID".to_string(), id.0.into());
    properties.insert("sharedstreetid".to_string(), id.0.into());

    // Left-to-right
    let mut slices = Vec::new();
    for (l, _) in r.children(false).into_iter().rev() {
        if let Some(mut slice) = lane(map.get_l(*l)) {
            slice
                .entry("direction".to_string())
                .or_insert("reverse".into());
            slices.push(serde_json::value::Value::Object(slice));
        }
    }
    for (l, _) in r.children(true).into_iter() {
        if let Some(mut slice) = lane(map.get_l(*l)) {
            slice
                .entry("direction".to_string())
                .or_insert("forward".into());
            slices.push(serde_json::value::Value::Object(slice));
        }
    }
    properties.insert(
        "slices".to_string(),
        serde_json::value::Value::Array(slices),
    );

    let gps_bounds = map.get_gps_bounds();
    Feature {
        bbox: None,
        geometry: Some(Geometry::new(Value::LineString(
            r.center_pts
                .points()
                .iter()
                .map(|pt| {
                    let gps = pt.to_gps(gps_bounds);
                    vec![gps.x(), gps.y()]
                })
                .collect(),
        ))),
        id: Some(Id::Number(id.0.into())),
        properties: Some(properties),
        foreign_members: None,
    }
}

fn lane(lane: &Lane) -> Option<serde_json::Map<String, serde_json::value::Value>> {
    let mut slice = serde_json::Map::new();
    // TODO We don't really model turn lanes yet; they'll all show up as drive_lane
    slice.insert(
        "type".to_string(),
        match lane.lane_type {
            LaneType::Driving => "drive_lane".into(),
            LaneType::Parking => "parking".into(),
            LaneType::Sidewalk => "sidewalk".into(),
            LaneType::Biking => "bike_lane".into(),
            LaneType::Bus => "bus_lane".into(),
            LaneType::SharedLeftTurn => "turn_lane".into(),
            LaneType::Construction => "construction_zone".into(),
            LaneType::LightRail => {
                return None;
            }
        },
    );
    if lane.lane_type == LaneType::SharedLeftTurn {
        slice.insert("direction".to_string(), "bidirectional".into());
    }
    slice.insert("width".to_string(), lane.width.inner_meters().into());
    slice.insert("height".to_string(), 0.0.into());
    // TODO Spec says required but shouldn't be
    slice.insert("material".to_string(), "asphalt".into());
    Some(slice)
}

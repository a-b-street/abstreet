use geojson::feature::Id;
use geojson::{Feature, FeatureCollection, GeoJson};

use map_model::{Direction, IntersectionID, Lane, LaneType, Map, RoadID};

/// Exports to https://github.com/d-wasserman/shared-row/, returns the filename
pub fn export(roads: Vec<RoadID>, intersections: Vec<IntersectionID>, map: &Map) -> String {
    let path = format!(
        "shared_row_export_{}.json",
        roads
            .iter()
            .take(5)
            .map(|r| r.0.to_string())
            .collect::<Vec<_>>()
            .join("_")
    );
    let mut features: Vec<Feature> = roads.into_iter().map(|r| road(r, map)).collect();
    for i in intersections {
        features.push(intersection(i, map));
    }
    let geojson = GeoJson::from(FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    });
    abstio::write_json(path.clone(), &geojson);
    path
}

fn road(id: RoadID, map: &Map) -> Feature {
    let r = map.get_r(id);
    let mut properties = serde_json::Map::new();
    // TODO Generate https://github.com/sharedstreets/sharedstreets-ref-system IDs
    properties.insert("OID".to_string(), id.0.into());
    properties.insert("sharedstreetid".to_string(), id.0.into());

    let mut slices = Vec::new();
    for (l, dir, _) in r.lanes_ltr() {
        if let Some(mut slice) = lane(map.get_l(l)) {
            slice
                .entry("direction".to_string())
                .or_insert(if dir == Direction::Fwd {
                    "forward".into()
                } else {
                    "reverse".into()
                });
            slices.push(serde_json::value::Value::Object(slice));
        }
    }
    properties.insert(
        "slices".to_string(),
        serde_json::value::Value::Array(slices),
    );

    Feature {
        bbox: None,
        geometry: Some(r.center_pts.to_geojson(Some(map.get_gps_bounds()))),
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
            // TODO Nope
            LaneType::Shoulder => "sidewalk".into(),
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

fn intersection(id: IntersectionID, map: &Map) -> Feature {
    let mut properties = serde_json::Map::new();
    properties.insert("intersection".to_string(), true.into());
    Feature {
        bbox: None,
        geometry: Some(
            map.get_i(id)
                .polygon
                .clone()
                .into_ring()
                .to_geojson(Some(map.get_gps_bounds())),
        ),
        id: Some(Id::Number(id.0.into())),
        properties: Some(properties),
        foreign_members: None,
    }
}

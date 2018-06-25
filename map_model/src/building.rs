// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use LaneType;
use Map;
use Pt2D;
use RoadID;
use geo;
use has_osm_tag;
use ordered_float::NotNaN;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct BuildingID(pub usize);

#[derive(Debug)]
pub struct Building {
    pub id: BuildingID,
    pub points: Vec<Pt2D>,
    pub osm_tags: Vec<String>,
    pub osm_way_id: i64,

    pub front_path: Option<(Pt2D, Pt2D)>,
}

impl PartialEq for Building {
    fn eq(&self, other: &Building) -> bool {
        self.id == other.id
    }
}

pub(crate) fn find_front_path(
    bldg_points: &Vec<Pt2D>,
    bldg_osm_tags: &Vec<String>,
    map: &Map,
) -> Option<(Pt2D, Pt2D)> {
    use geo::prelude::{ClosestPoint, EuclideanDistance};

    if let Some(tag) = bldg_osm_tags
        .iter()
        .find(|kv| kv.starts_with("addr:street="))
    {
        let (_, street_name) = tag.split_at("addr:street=".len());

        // TODO start from the side of the building, not the center
        let bldg_center = center(bldg_points);
        let center_pt = geo::Point::new(bldg_center.x(), bldg_center.y());

        // Find all matching sidewalks with that street name, then find the closest point on
        // that sidewalk
        let candidates: Vec<(RoadID, geo::Point<f64>)> = map.all_roads()
            .iter()
            .filter_map(|r| {
                if r.lane_type == LaneType::Sidewalk
                    && has_osm_tag(&r.osm_tags, "name", street_name)
                {
                    if let geo::Closest::SinglePoint(pt) =
                        road_to_line_string(r.id, map).closest_point(&center_pt)
                    {
                        return Some((r.id, pt));
                    }
                }
                None
            })
            .collect();

        if let Some(closest) = candidates
            .iter()
            .min_by_key(|pair| NotNaN::new(pair.1.euclidean_distance(&center_pt)).unwrap())
        {
            return Some((bldg_center, Pt2D::new(closest.1.x(), closest.1.y())));
        }
    }
    None
}

fn center(pts: &Vec<Pt2D>) -> Pt2D {
    let mut x = 0.0;
    let mut y = 0.0;
    for pt in pts {
        x += pt.x();
        y += pt.y();
    }
    let len = pts.len() as f64;
    Pt2D::new(x / len, y / len)
}

fn road_to_line_string(r: RoadID, map: &Map) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = map.get_r(r)
        .lane_center_lines
        .iter()
        .flat_map(|pair| {
            vec![
                geo::Point::new(pair.0.x(), pair.0.y()),
                geo::Point::new(pair.1.x(), pair.1.y()),
            ]
        })
        .collect();
    pts.into()
}

use dimensioned::si;
use geom::{Bounds, HashablePt2D, LonLat};
use std::collections::BTreeMap;

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Map {
    pub roads: Vec<Road>,
    pub intersections: Vec<Intersection>,
    pub buildings: Vec<Building>,
    pub parcels: Vec<Parcel>,
}

impl Map {
    pub fn blank() -> Map {
        Map {
            roads: Vec::new(),
            intersections: Vec::new(),
            buildings: Vec::new(),
            parcels: Vec::new(),
        }
    }

    pub fn get_gps_bounds(&self) -> Bounds {
        let mut bounds = Bounds::new();

        for r in &self.roads {
            for pt in &r.points {
                bounds.update_coord(pt);
            }
        }
        for i in &self.intersections {
            bounds.update_coord(&i.point);
        }
        for b in &self.buildings {
            for pt in &b.points {
                bounds.update_coord(pt);
            }
        }
        for p in &self.parcels {
            for pt in &p.points {
                bounds.update_coord(pt);
            }
        }

        bounds
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Road {
    pub points: Vec<LonLat>,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
}

impl Road {
    pub fn first_pt(&self) -> HashablePt2D {
        self.points[0].to_hashable()
    }

    pub fn last_pt(&self) -> HashablePt2D {
        self.points.last().unwrap().to_hashable()
    }
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Intersection {
    pub point: LonLat,
    pub elevation: si::Meter<f64>,
    pub has_traffic_signal: bool,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Building {
    // last point never the first?
    pub points: Vec<LonLat>,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Parcel {
    // last point never the first?
    pub points: Vec<LonLat>,
    // TODO decide what metadata from the shapefile is useful
}

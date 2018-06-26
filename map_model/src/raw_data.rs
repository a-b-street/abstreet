use Bounds;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LatLon {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Road {
    pub points: Vec<LatLon>,
    pub osm_tags: HashMap<String, String>,
    pub osm_way_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Intersection {
    pub point: LatLon,
    pub elevation_meters: f64,
    pub has_traffic_signal: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    // last point never the first?
    pub points: Vec<LatLon>,
    pub osm_tags: HashMap<String, String>,
    pub osm_way_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Parcel {
    // last point never the first?
    pub points: Vec<LatLon>,
    // TODO decide what metadata from the shapefile is useful
}

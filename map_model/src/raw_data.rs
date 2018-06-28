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

// longitude is x, latitude is y
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LonLat {
    pub longitude: f64,
    pub latitude: f64,
}

impl LonLat {
    pub fn new(lon: f64, lat: f64) -> LonLat {
        LonLat {
            longitude: lon,
            latitude: lat,
        }
    }

    // TODO use dimensioned?
    pub fn gps_dist_meters(&self, other: LonLat) -> f64 {
        // Haversine distance
        let earth_radius_m = 6371000.0;
        let lon1 = self.longitude.to_radians();
        let lon2 = other.longitude.to_radians();
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();

        let delta_lat = lat2 - lat1;
        let delta_lon = lon2 - lon1;

        let a = (delta_lat / 2.0).sin().powi(2)
            + (delta_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        earth_radius_m * c
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Road {
    pub points: Vec<LonLat>,
    pub osm_tags: HashMap<String, String>,
    pub osm_way_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Intersection {
    pub point: LonLat,
    pub elevation_meters: f64,
    pub has_traffic_signal: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    // last point never the first?
    pub points: Vec<LonLat>,
    pub osm_tags: HashMap<String, String>,
    pub osm_way_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Parcel {
    // last point never the first?
    pub points: Vec<LonLat>,
    // TODO decide what metadata from the shapefile is useful
}

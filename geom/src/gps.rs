use pt::HashablePt2D;

// longitude is x, latitude is y
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
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

    pub fn to_hashable(&self) -> HashablePt2D {
        HashablePt2D::new(self.longitude, self.latitude)
    }
}

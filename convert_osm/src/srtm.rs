use byteorder::{BigEndian, ReadBytesExt};
use geom::{Distance, LonLat};
use std::fs::File;
use std::io;

// TODO Make sure this actually works, and link to references describing the format.
// - It's VERY wrong! Look at 19th and Boyer, and Delmar.
// TODO Use http://gis.ess.washington.edu/data/raster/tenmeter/byquad/seattle/index.html or
// something else instead?

// Assuming the 3-second arcs
const GRID_DIM: usize = 1201;

pub struct Elevation {
    lon_offset: f64,
    lat_offset: f64,
    data: Vec<i16>,
}

impl Elevation {
    pub fn load(path: &str) -> Result<Elevation, io::Error> {
        println!("Reading elevation data from {}", path);
        let mut f = File::open(path).unwrap();

        let mut e = Elevation {
            // TODO dont hardcode
            lon_offset: -122.0,
            lat_offset: 47.0,
            data: Vec::with_capacity(GRID_DIM.pow(2)),
        };
        // TODO off by one?
        for _ in 0..GRID_DIM.pow(2) {
            e.data.push(f.read_i16::<BigEndian>().unwrap());
        }
        Ok(e)
    }

    pub fn get(&self, pt: LonLat) -> Distance {
        // TODO assert the (lon, lat) match the offsets
        // TODO not tons of confidence in any of this.
        // TODO interpolate from the 4 matching tiles?
        let x = ((pt.x() - self.lon_offset).abs() * (GRID_DIM as f64)) as usize;
        let y = ((pt.y() - self.lat_offset).abs() * (GRID_DIM as f64)) as usize;
        let i = x + (y * GRID_DIM);
        Distance::meters(f64::from(self.data[i]))
    }
}

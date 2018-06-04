// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use map_model::Pt2D;
use std::io::Error;

pub struct TrafficSignal {
    // One signal may cover several intersections that're close together.
    pub intersections: Vec<Pt2D>,
}

pub fn extract(path: &str) -> Result<Vec<TrafficSignal>, Error> {
    println!("Opening {}", path);
    // TODO Read the .shp
    Ok(Vec::new())
}

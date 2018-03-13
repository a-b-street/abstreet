// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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

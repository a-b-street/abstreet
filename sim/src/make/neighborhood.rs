use abstutil;
use geom::{GPSBounds, LonLat, Polygon, Pt2D};
use map_model::{BuildingID, Map, RoadID};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Error, Write};

// This form is used by the editor plugin to edit and for serialization. Storing points in GPS is
// more compatible with slight changes to the bounding box of a map over time.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NeighborhoodBuilder {
    pub map_name: String,
    pub name: String,
    pub points: Vec<LonLat>,
}

impl NeighborhoodBuilder {
    pub fn finalize(&self, gps_bounds: &GPSBounds) -> Neighborhood {
        assert!(self.points.len() >= 3);
        Neighborhood {
            map_name: self.map_name.clone(),
            name: self.name.clone(),
            polygon: Polygon::new(
                &self
                    .points
                    .iter()
                    .map(|pt| {
                        Pt2D::from_gps(*pt, gps_bounds)
                            .expect(&format!("Polygon {} has bad pt {}", self.name, pt))
                    })
                    .collect(),
            ),
        }
    }

    pub fn save(&self) {
        abstutil::save_object("neighborhoods", &self.map_name, &self.name, self);
    }

    // https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format
    pub fn save_as_osmosis(&self) -> Result<(), Error> {
        let path = format!("../data/polygons/{}.poly", self.name);
        let mut f = File::create(&path)?;

        write!(f, "{}\n", self.name);
        write!(f, "1\n");
        for gps in &self.points {
            write!(f, "     {}    {}\n", gps.longitude, gps.latitude);
        }
        // Have to repeat the first point
        {
            write!(
                f,
                "     {}    {}\n",
                self.points[0].longitude, self.points[0].latitude
            );
        }
        write!(f, "END\n");
        write!(f, "END\n");

        println!("Exported {}", path);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Neighborhood {
    pub map_name: String,
    pub name: String,
    pub polygon: Polygon,
}

impl Neighborhood {
    pub fn load_all(map_name: &str, gps_bounds: &GPSBounds) -> Vec<(String, Neighborhood)> {
        abstutil::load_all_objects::<NeighborhoodBuilder>("neighborhoods", map_name)
            .into_iter()
            .map(|(name, builder)| (name, builder.finalize(gps_bounds)))
            .collect()
    }

    // TODO This should use quadtrees and/or not just match the center of each building.
    pub fn find_matching_buildings(&self, map: &Map) -> Vec<BuildingID> {
        let mut results: Vec<BuildingID> = Vec::new();
        for b in map.all_buildings() {
            if self.polygon.contains_pt(Pt2D::center(&b.points)) {
                results.push(b.id);
            }
        }
        results
    }

    // TODO This should use quadtrees and/or not just match one point of each road.
    pub fn find_matching_roads(&self, map: &Map) -> BTreeSet<RoadID> {
        let mut results: BTreeSet<RoadID> = BTreeSet::new();
        for r in map.all_roads() {
            if self.polygon.contains_pt(r.center_pts.first_pt()) {
                results.insert(r.id);
            }
        }
        results
    }

    pub fn make_everywhere(map: &Map) -> Neighborhood {
        let bounds = map.get_bounds();

        Neighborhood {
            map_name: map.get_name().to_string(),
            name: "_everywhere_".to_string(),
            polygon: Polygon::new(&vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(bounds.max_x, 0.0),
                Pt2D::new(bounds.max_x, bounds.max_y),
                Pt2D::new(0.0, bounds.max_y),
                Pt2D::new(0.0, 0.0),
            ]),
        }
    }
}

use crate::{BuildingID, Map, RoadID};
use aabb_quadtree::QuadTree;
use geom::{GPSBounds, LonLat, Polygon, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
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
        abstutil::write_json(
            abstutil::path_neighborhood(&self.map_name, &self.name),
            self,
        );
    }

    // https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format
    pub fn save_as_osmosis(&self) -> Result<(), Error> {
        let path = abstutil::path_polygon(&self.name);
        let mut f = File::create(&path)?;

        writeln!(f, "{}", self.name)?;
        writeln!(f, "1")?;
        for gps in &self.points {
            writeln!(f, "     {}    {}", gps.x(), gps.y())?;
        }
        // Have to repeat the first point
        {
            writeln!(f, "     {}    {}", self.points[0].x(), self.points[0].y())?;
        }
        writeln!(f, "END")?;
        writeln!(f, "END")?;

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
        abstutil::load_all_objects::<NeighborhoodBuilder>(abstutil::path_all_neighborhoods(
            map_name,
        ))
        .into_iter()
        .map(|(name, builder)| (name, builder.finalize(gps_bounds)))
        .collect()
    }

    fn make_everywhere(map: &Map) -> Neighborhood {
        Neighborhood {
            map_name: map.get_name().to_string(),
            name: "_everywhere_".to_string(),
            polygon: map.get_bounds().get_rectangle(),
        }
    }
}

pub struct FullNeighborhoodInfo {
    pub name: String,
    pub buildings: Vec<BuildingID>,
    pub roads: BTreeSet<RoadID>,
}

impl FullNeighborhoodInfo {
    pub fn load_all(map: &Map) -> HashMap<String, FullNeighborhoodInfo> {
        let mut neighborhoods = Neighborhood::load_all(map.get_name(), map.get_gps_bounds());
        neighborhoods.push((
            "_everywhere_".to_string(),
            Neighborhood::make_everywhere(map),
        ));

        let mut bldg_quadtree = QuadTree::default(map.get_bounds().as_bbox());
        for b in map.all_buildings() {
            bldg_quadtree.insert_with_box(b.id, b.polygon.get_bounds().as_bbox());
        }
        let mut road_quadtree = QuadTree::default(map.get_bounds().as_bbox());
        for r in map.all_roads() {
            road_quadtree.insert_with_box(
                r.id,
                r.get_thick_polygon(map).unwrap().get_bounds().as_bbox(),
            );
        }

        let mut full_info = HashMap::new();
        for (name, n) in &neighborhoods {
            let mut info = FullNeighborhoodInfo {
                name: name.to_string(),
                buildings: Vec::new(),
                roads: BTreeSet::new(),
            };

            for &(id, _, _) in &bldg_quadtree.query(n.polygon.get_bounds().as_bbox()) {
                // TODO Polygon containment is hard; just see if the center is inside.
                if n.polygon.contains_pt(map.get_b(*id).polygon.center()) {
                    info.buildings.push(*id);
                }
            }

            for &(id, _, _) in &road_quadtree.query(n.polygon.get_bounds().as_bbox()) {
                // TODO Polygon containment is hard; just see if the "center" of each endpoint is
                // inside.
                let r = map.get_r(*id);
                let pt1 = r.center_pts.first_pt();
                let pt2 = r.center_pts.last_pt();
                if n.polygon.contains_pt(pt1) && n.polygon.contains_pt(pt2) {
                    info.roads.insert(*id);
                }
            }

            full_info.insert(name.to_string(), info);
        }
        full_info
    }
}

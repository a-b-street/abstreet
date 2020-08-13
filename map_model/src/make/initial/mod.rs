mod geometry;
pub mod lane_specs;

pub use self::geometry::intersection_polygon;
use crate::raw::{DrivingSide, OriginalRoad, RawMap, RawRoad};
use crate::{osm, IntersectionType};
use abstutil::{Tags, Timer};
use geom::{Bounds, Circle, Distance, PolyLine, Polygon, Pt2D};
use lane_specs::LaneSpec;
use std::collections::{BTreeMap, BTreeSet};

pub struct InitialMap {
    pub roads: BTreeMap<OriginalRoad, Road>,
    pub intersections: BTreeMap<osm::NodeID, Intersection>,

    pub bounds: Bounds,
}

pub struct Road {
    // Redundant but useful to embed
    pub id: OriginalRoad,
    pub src_i: osm::NodeID,
    pub dst_i: osm::NodeID,
    // The true center of the road, including sidewalks
    pub trimmed_center_pts: PolyLine,
    pub half_width: Distance,
    pub lane_specs: Vec<LaneSpec>,
    pub osm_tags: Tags,
}

impl Road {
    pub fn new(id: OriginalRoad, r: &RawRoad, driving_side: DrivingSide) -> Road {
        let lane_specs = lane_specs::get_lane_specs(&r.osm_tags);
        let (trimmed_center_pts, total_width) = r.get_geometry(id, driving_side);

        Road {
            id,
            src_i: id.i1,
            dst_i: id.i2,
            trimmed_center_pts,
            half_width: total_width / 2.0,
            lane_specs,
            osm_tags: r.osm_tags.clone(),
        }
    }
}

pub struct Intersection {
    // Redundant but useful to embed
    pub id: osm::NodeID,
    pub polygon: Polygon,
    pub roads: BTreeSet<OriginalRoad>,
    pub intersection_type: IntersectionType,
    pub elevation: Distance,
}

impl InitialMap {
    pub fn new(raw: &RawMap, bounds: &Bounds, timer: &mut Timer) -> InitialMap {
        let mut m = InitialMap {
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            bounds: bounds.clone(),
        };

        for (id, i) in &raw.intersections {
            m.intersections.insert(
                *id,
                Intersection {
                    id: *id,
                    // Dummy thing to start with
                    polygon: Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(1.0)).to_polygon(),
                    roads: BTreeSet::new(),
                    intersection_type: i.intersection_type,
                    elevation: i.elevation,
                },
            );
        }

        for (id, r) in &raw.roads {
            if id.i1 == id.i2 {
                timer.warn(format!("Skipping loop {}", id));
                continue;
            }
            if PolyLine::new(r.center_points.clone()).is_err() {
                timer.warn(format!("Skipping broken geom {}", id));
                continue;
            }

            m.intersections.get_mut(&id.i1).unwrap().roads.insert(*id);
            m.intersections.get_mut(&id.i2).unwrap().roads.insert(*id);

            m.roads
                .insert(*id, Road::new(*id, r, raw.config.driving_side));
        }

        timer.start_iter("find each intersection polygon", m.intersections.len());
        for i in m.intersections.values_mut() {
            timer.next();
            match intersection_polygon(raw.config.driving_side, i, &mut m.roads, timer) {
                Ok((poly, _)) => {
                    i.polygon = poly;
                }
                Err(err) => {
                    timer.error(format!(
                        "Can't make intersection geometry for {}: {}",
                        i.id, err
                    ));

                    // Don't trim lines back at all
                    let r = &m.roads[i.roads.iter().next().unwrap()];
                    let pt = if r.src_i == i.id {
                        r.trimmed_center_pts.first_pt()
                    } else {
                        r.trimmed_center_pts.last_pt()
                    };
                    i.polygon = Circle::new(pt, Distance::meters(3.0)).to_polygon();

                    // Also don't attempt to make TurnGroups later!
                    i.intersection_type = IntersectionType::StopSign;
                }
            }
        }

        // Some roads near borders get completely squished. Stretch them out here. Attempting to do
        // this in the convert_osm layer doesn't work, because predicting how much roads will be
        // trimmed is impossible.
        let min_len = Distance::meters(5.0);
        for i in m.intersections.values_mut() {
            if i.intersection_type != IntersectionType::Border {
                continue;
            }
            let r = m.roads.get_mut(i.roads.iter().next().unwrap()).unwrap();
            if r.trimmed_center_pts.length() >= min_len {
                continue;
            }
            if r.dst_i == i.id {
                r.trimmed_center_pts = r.trimmed_center_pts.extend_to_length(min_len);
            } else {
                r.trimmed_center_pts = r
                    .trimmed_center_pts
                    .reversed()
                    .extend_to_length(min_len)
                    .reversed();
            }
            i.polygon = intersection_polygon(raw.config.driving_side, i, &mut m.roads, timer)
                .unwrap()
                .0;
            timer.note(format!(
                "Shifted border {} out a bit to make the road a reasonable length",
                i.id
            ));
        }

        m
    }
}

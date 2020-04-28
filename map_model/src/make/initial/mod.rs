mod geometry;
pub mod lane_specs;

pub use self::geometry::intersection_polygon;
use crate::raw::{OriginalIntersection, OriginalRoad, RawMap, RawRoad};
use crate::{IntersectionType, LaneType, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};
use abstutil::Timer;
use geom::{Bounds, Distance, PolyLine, Pt2D};
use std::collections::{BTreeMap, BTreeSet};

pub struct InitialMap {
    pub roads: BTreeMap<OriginalRoad, Road>,
    pub intersections: BTreeMap<OriginalIntersection, Intersection>,

    pub bounds: Bounds,
}

pub struct Road {
    // Redundant but useful to embed
    pub id: OriginalRoad,
    pub src_i: OriginalIntersection,
    pub dst_i: OriginalIntersection,
    pub original_center_pts: PolyLine,
    pub trimmed_center_pts: PolyLine,
    pub fwd_width: Distance,
    pub back_width: Distance,
    pub lane_specs: Vec<LaneSpec>,
}

impl Road {
    pub fn new(id: OriginalRoad, r: &RawRoad) -> Road {
        let lane_specs = get_lane_specs(&r.osm_tags);
        let mut fwd_width = Distance::ZERO;
        let mut back_width = Distance::ZERO;
        for l in &lane_specs {
            let w = if l.lane_type == LaneType::Sidewalk {
                SIDEWALK_THICKNESS
            } else {
                NORMAL_LANE_THICKNESS
            };
            if l.reverse_pts {
                back_width += w;
            } else {
                fwd_width += w;
            }
        }

        let center_pts = PolyLine::new(r.center_points.clone());
        Road {
            id,
            src_i: id.i1,
            dst_i: id.i2,
            original_center_pts: center_pts.clone(),
            trimmed_center_pts: center_pts,
            fwd_width,
            back_width,
            lane_specs,
        }
    }
}

pub struct Intersection {
    // Redundant but useful to embed
    pub id: OriginalIntersection,
    pub polygon: Vec<Pt2D>,
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
                    polygon: Vec::new(),
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
            m.intersections.get_mut(&id.i1).unwrap().roads.insert(*id);
            m.intersections.get_mut(&id.i2).unwrap().roads.insert(*id);

            m.roads.insert(*id, Road::new(*id, r));
        }

        timer.start_iter("find each intersection polygon", m.intersections.len());
        for i in m.intersections.values_mut() {
            timer.next();

            i.polygon = intersection_polygon(raw.driving_side, i, &mut m.roads, timer).0;
        }

        m
    }
}

pub struct LaneSpec {
    pub lane_type: LaneType,
    pub reverse_pts: bool,
}

pub fn get_lane_specs(osm_tags: &BTreeMap<String, String>) -> Vec<LaneSpec> {
    let (side1_types, side2_types) = lane_specs::get_lane_types(osm_tags);

    let mut specs: Vec<LaneSpec> = Vec::new();
    for lane_type in side1_types {
        specs.push(LaneSpec {
            lane_type,
            reverse_pts: false,
        });
    }
    for lane_type in side2_types {
        specs.push(LaneSpec {
            lane_type,
            reverse_pts: true,
        });
    }
    if specs.is_empty() {
        panic!("Road with tags {:?} wound up with no lanes!", osm_tags);
    }
    specs
}

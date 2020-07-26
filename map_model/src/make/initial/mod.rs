mod geometry;
pub mod lane_specs;

pub use self::geometry::intersection_polygon;
use crate::raw::{DrivingSide, OriginalIntersection, OriginalRoad, RawMap, RawRoad};
use crate::{IntersectionType, LaneType, NORMAL_LANE_THICKNESS, SIDEWALK_THICKNESS};
use abstutil::{Tags, Timer};
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
    // The true center of the road, including sidewalks
    pub trimmed_center_pts: PolyLine,
    pub half_width: Distance,
    pub lane_specs: Vec<LaneSpec>,
}

impl Road {
    pub fn new(id: OriginalRoad, r: &RawRoad, driving_side: DrivingSide) -> Road {
        let lane_specs = get_lane_specs(&r.osm_tags);
        let mut total_width = Distance::ZERO;
        let mut sidewalk_right = false;
        let mut sidewalk_left = false;
        for l in &lane_specs {
            total_width += l.width();
            if l.lane_type == LaneType::Sidewalk {
                if l.reverse_pts {
                    sidewalk_left = true;
                } else {
                    sidewalk_right = true;
                }
            }
        }

        // If there's a sidewalk on only one side, adjust the true center of the road.
        let mut trimmed_center_pts = PolyLine::new(r.center_points.clone()).expect(&id.to_string());
        if sidewalk_right && !sidewalk_left {
            trimmed_center_pts =
                driving_side.right_shift(trimmed_center_pts, SIDEWALK_THICKNESS / 2.0);
        } else if sidewalk_left && !sidewalk_right {
            trimmed_center_pts =
                driving_side.left_shift(trimmed_center_pts, SIDEWALK_THICKNESS / 2.0);
        }

        Road {
            id,
            src_i: id.i1,
            dst_i: id.i2,
            trimmed_center_pts,
            half_width: total_width / 2.0,
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

            i.polygon = intersection_polygon(raw.config.driving_side, i, &mut m.roads, timer).0;
        }

        m
    }
}

pub struct LaneSpec {
    pub lane_type: LaneType,
    pub reverse_pts: bool,
}

impl LaneSpec {
    pub fn width(&self) -> Distance {
        if self.lane_type == LaneType::Sidewalk {
            SIDEWALK_THICKNESS
        } else {
            NORMAL_LANE_THICKNESS
        }
    }
}

pub fn get_lane_specs(osm_tags: &Tags) -> Vec<LaneSpec> {
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

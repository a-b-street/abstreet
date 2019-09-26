mod geometry;
pub mod lane_specs;

pub use self::geometry::intersection_polygon;
use crate::raw::{RawMap, StableIntersectionID, StableRoadID};
use crate::{IntersectionType, LaneType, LANE_THICKNESS};
use abstutil::Timer;
use geom::{Bounds, Distance, PolyLine, Pt2D};
use std::collections::{BTreeMap, BTreeSet};

pub struct InitialMap {
    pub roads: BTreeMap<StableRoadID, Road>,
    pub intersections: BTreeMap<StableIntersectionID, Intersection>,

    pub name: String,
    pub bounds: Bounds,
}

pub struct Road {
    pub id: StableRoadID,
    pub src_i: StableIntersectionID,
    pub dst_i: StableIntersectionID,
    pub original_center_pts: PolyLine,
    pub trimmed_center_pts: PolyLine,
    pub fwd_width: Distance,
    pub back_width: Distance,
    pub lane_specs: Vec<LaneSpec>,
}

impl Road {
    pub fn original_endpoint(&self, i: StableIntersectionID) -> Pt2D {
        if self.src_i == i {
            self.original_center_pts.first_pt()
        } else if self.dst_i == i {
            self.original_center_pts.last_pt()
        } else {
            panic!("{} doesn't end at {}", self.id, i);
        }
    }
}

pub struct Intersection {
    pub id: StableIntersectionID,
    pub polygon: Vec<Pt2D>,
    pub roads: BTreeSet<StableRoadID>,
    pub intersection_type: IntersectionType,
}

impl InitialMap {
    pub fn new(name: String, raw: &RawMap, bounds: &Bounds, timer: &mut Timer) -> InitialMap {
        let mut m = InitialMap {
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            name,
            bounds: bounds.clone(),
        };

        for (stable_id, i) in &raw.intersections {
            m.intersections.insert(
                *stable_id,
                Intersection {
                    id: *stable_id,
                    polygon: Vec::new(),
                    roads: BTreeSet::new(),
                    intersection_type: i.intersection_type,
                },
            );
        }

        for (stable_id, r) in &raw.roads {
            if r.i1 == r.i2 {
                timer.warn(format!(
                    "OSM way {} is a loop on {}, skipping what would've been {}",
                    r.orig_id.osm_way_id, r.i1, stable_id
                ));
                continue;
            }
            m.intersections
                .get_mut(&r.i1)
                .unwrap()
                .roads
                .insert(*stable_id);
            m.intersections
                .get_mut(&r.i2)
                .unwrap()
                .roads
                .insert(*stable_id);

            let lane_specs = get_lane_specs(&r.osm_tags, *stable_id);
            let mut fwd_width = Distance::ZERO;
            let mut back_width = Distance::ZERO;
            for l in &lane_specs {
                if l.reverse_pts {
                    back_width += LANE_THICKNESS;
                } else {
                    fwd_width += LANE_THICKNESS;
                }
            }

            // TODO I can't find anything online that describes how to interpret the given OSM
            // geometry of one-ways. I'm interpreting the way as the edge of the road (and only
            // shift_right()ing from there). But could also uncomment this and interpret the way as
            // the actual center of the one-way road. It looks quite bad -- dual carriageways get
            // smooshed together.
            /*assert_ne!(fwd_width, Distance::ZERO);
            if back_width == Distance::ZERO {
                // Interpret the original OSM geometry of one-ways as the actual center of the
                // road.
                original_center_pts = original_center_pts.shift_left(fwd_width / 2.0);
            }*/

            let center_pts = PolyLine::new(r.center_points.clone());
            m.roads.insert(
                *stable_id,
                Road {
                    id: *stable_id,
                    src_i: r.i1,
                    dst_i: r.i2,
                    original_center_pts: center_pts.clone(),
                    trimmed_center_pts: center_pts,
                    fwd_width,
                    back_width,
                    lane_specs,
                },
            );
        }

        timer.start_iter("find each intersection polygon", m.intersections.len());
        for i in m.intersections.values_mut() {
            timer.next();

            i.polygon = intersection_polygon(i, &mut m.roads, timer).0;
        }

        m
    }
}

pub struct LaneSpec {
    pub lane_type: LaneType,
    pub reverse_pts: bool,
}

pub fn get_lane_specs(osm_tags: &BTreeMap<String, String>, id: StableRoadID) -> Vec<LaneSpec> {
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
        panic!(
            "Road with tags {:?} wound up with no lanes! {:?}",
            id, osm_tags
        );
    }
    specs
}

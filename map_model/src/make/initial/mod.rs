mod geometry;
pub mod lane_specs;
mod merge;

use crate::raw_data::{StableIntersectionID, StableRoadID};
use crate::{osm, raw_data, IntersectionType, LaneType, LANE_THICKNESS};
use abstutil::{deserialize_btreemap, serialize_btreemap, Timer};
use geom::{Bounds, Distance, PolyLine, Pt2D};
use serde_derive::{Deserialize, Serialize};
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
    // Copied here from the raw layer, because merge_degenerate_intersection needs to modify this.
    // TODO Maybe don't need this now?
    pub osm_tags: BTreeMap<String, String>,
    pub override_turn_restrictions_to: Vec<StableRoadID>,
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

    pub fn has_parking(&self) -> bool {
        self.osm_tags.get(osm::PARKING_LANE_FWD) == Some(&"true".to_string())
            || self.osm_tags.get(osm::PARKING_LANE_BACK) == Some(&"true".to_string())
    }

    pub fn reset_pts_on_side(&mut self, i: StableIntersectionID) {
        if self.dst_i == i {
            if let Some(append) = self
                .original_center_pts
                .get_slice_starting_at(self.trimmed_center_pts.last_pt())
            {
                self.trimmed_center_pts = self.trimmed_center_pts.clone().extend(append);
            }
        } else {
            if let Some(prepend) = self
                .original_center_pts
                .get_slice_ending_at(self.trimmed_center_pts.first_pt())
            {
                self.trimmed_center_pts = prepend.extend(self.trimmed_center_pts.clone());
            }
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
    pub fn new(
        name: String,
        data: &raw_data::Map,
        bounds: &Bounds,
        timer: &mut Timer,
    ) -> InitialMap {
        let mut m = InitialMap {
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            name,
            bounds: bounds.clone(),
        };

        for (stable_id, i) in &data.intersections {
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

        for (stable_id, r) in &data.roads {
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
                    osm_tags: r.osm_tags.clone(),
                    override_turn_restrictions_to: Vec::new(),
                },
            );
        }

        timer.start_iter("find each intersection polygon", m.intersections.len());
        for i in m.intersections.values_mut() {
            timer.next();

            i.polygon = geometry::intersection_polygon(i, &mut m.roads, timer);
        }

        merge::short_roads(&mut m, timer);

        m
    }

    pub fn merge_road(&mut self, r: StableRoadID, timer: &mut Timer) {
        merge::merge(self, r, timer);
    }

    pub fn delete_road(&mut self, r: StableRoadID, timer: &mut Timer) {
        let road = self.roads.remove(&r).unwrap();
        {
            let mut i = self.intersections.get_mut(&road.src_i).unwrap();
            i.roads.remove(&r);
            i.polygon = geometry::intersection_polygon(i, &mut self.roads, timer);
        }
        {
            let mut i = self.intersections.get_mut(&road.dst_i).unwrap();
            i.roads.remove(&r);
            i.polygon = geometry::intersection_polygon(i, &mut self.roads, timer);
        }
    }

    pub fn merge_degenerate_intersection(
        &mut self,
        delete_i: StableIntersectionID,
        timer: &mut Timer,
    ) {
        let (r1, r2) = {
            let i = self.intersections.remove(&delete_i).unwrap();
            let roads: Vec<StableRoadID> = i.roads.into_iter().collect();
            assert_eq!(roads.len(), 2);
            (roads[0], roads[1])
        };
        // new_i1 is the other end of r1, new_i2 is the other end of r2
        let new_i1 = {
            let r = &self.roads[&r1];
            if r.src_i == delete_i {
                r.dst_i
            } else {
                r.src_i
            }
        };
        let new_i2 = {
            let r = &self.roads[&r2];
            if r.src_i == delete_i {
                r.dst_i
            } else {
                r.src_i
            }
        };

        // Arbitrarily delete r1. Fix up intersections
        let deleted_road = self.roads.remove(&r1).unwrap();
        {
            let i = self.intersections.get_mut(&new_i1).unwrap();
            i.roads.remove(&r1);
            i.roads.insert(r2);
        }
        // Start at delete_i and go to new_i1.
        let pts_towards_new_i1 = if deleted_road.src_i == delete_i {
            deleted_road.original_center_pts
        } else {
            deleted_road.original_center_pts.reversed()
        };

        // Fix up r2.
        {
            let r = self.roads.get_mut(&r2).unwrap();
            if r.src_i == delete_i {
                r.src_i = new_i1;
                r.original_center_pts = pts_towards_new_i1
                    .reversed()
                    .extend(r.original_center_pts.clone());
            } else {
                r.dst_i = new_i1;
                r.original_center_pts = r.original_center_pts.clone().extend(pts_towards_new_i1);
            }
            r.trimmed_center_pts = r.original_center_pts.clone();
        }

        // Redo the intersection geometry.
        {
            let i = self.intersections.get_mut(&new_i1).unwrap();
            i.polygon = geometry::intersection_polygon(i, &mut self.roads, timer);
        }
        {
            let i = self.intersections.get_mut(&new_i2).unwrap();
            i.polygon = geometry::intersection_polygon(i, &mut self.roads, timer);
        }

        // Preserve some OSM tags.
        {
            let r = self.roads.get_mut(&r2).unwrap();
            for (k, v) in deleted_road.osm_tags {
                if !r.osm_tags.contains_key(&k) {
                    r.osm_tags.insert(k, v);
                }
            }
        }
    }

    pub fn move_intersection(&mut self, i: StableIntersectionID, pt: Pt2D, timer: &mut Timer) {
        for r in &self.intersections[&i].roads {
            let road = self.roads.get_mut(r).unwrap();
            road.reset_pts_on_side(i);
            let mut orig_pts = road.original_center_pts.points().clone();
            let mut trimmed_pts = road.trimmed_center_pts.points().clone();
            if road.src_i == i {
                orig_pts[0] = pt;
                trimmed_pts[0] = pt;
            } else if road.dst_i == i {
                trimmed_pts.pop();
                trimmed_pts.push(pt);

                orig_pts.pop();
                orig_pts.push(pt);
            } else {
                unreachable!()
            }
            road.trimmed_center_pts = PolyLine::new(trimmed_pts);
            road.original_center_pts = PolyLine::new(orig_pts);
        }

        // TODO Also fix up the other intersections... make sure to just do it once though!

        let intersection = self.intersections.get_mut(&i).unwrap();
        intersection.polygon = geometry::intersection_polygon(intersection, &mut self.roads, timer);
    }

    pub fn override_parking(&mut self, r: StableRoadID, has_parking: bool, timer: &mut Timer) {
        let (src_i, dst_i) = {
            let mut road = self.roads.get_mut(&r).unwrap();
            if has_parking {
                road.osm_tags
                    .insert(osm::PARKING_LANE_FWD.to_string(), "true".to_string());
                road.osm_tags
                    .insert(osm::PARKING_LANE_BACK.to_string(), "true".to_string());
            } else {
                road.osm_tags.remove(osm::PARKING_LANE_FWD);
                road.osm_tags.remove(osm::PARKING_LANE_BACK);
            }

            let lane_specs = get_lane_specs(&road.osm_tags, r);
            let mut fwd_width = Distance::ZERO;
            let mut back_width = Distance::ZERO;
            for l in &lane_specs {
                if l.reverse_pts {
                    back_width += LANE_THICKNESS;
                } else {
                    fwd_width += LANE_THICKNESS;
                }
            }
            road.lane_specs = lane_specs;
            road.fwd_width = fwd_width;
            road.back_width = back_width;

            (road.src_i, road.dst_i)
        };

        // Reset to original_center_pts (on one side) for all roads connected to both
        // intersections.
        for i in &[src_i, dst_i] {
            for r in &self.intersections[i].roads {
                self.roads.get_mut(r).unwrap().reset_pts_on_side(*i);
            }
        }

        {
            let mut i = self.intersections.get_mut(&src_i).unwrap();
            i.polygon = geometry::intersection_polygon(i, &mut self.roads, timer);
        }
        {
            let mut i = self.intersections.get_mut(&dst_i).unwrap();
            i.polygon = geometry::intersection_polygon(i, &mut self.roads, timer);
        }
    }

    pub fn apply_hints(&mut self, hints: &Hints, raw: &raw_data::Map, timer: &mut Timer) {
        timer.start_iter("apply hints", hints.hints.len());
        let mut cnt = 0;
        for h in &hints.hints {
            timer.next();
            match h {
                Hint::MergeRoad(orig) => {
                    if let Some(r) = raw.find_r(*orig) {
                        cnt += 1;
                        self.merge_road(r, timer);
                    }
                }
                Hint::DeleteRoad(orig) => {
                    if let Some(r) = raw.find_r(*orig) {
                        cnt += 1;
                        self.delete_road(r, timer);
                    }
                }
                Hint::MergeDegenerateIntersection(orig) => {
                    if let Some(i) = raw.find_i(*orig) {
                        cnt += 1;
                        self.merge_degenerate_intersection(i, timer);
                    }
                }
                Hint::BanTurnsBetween(orig1, orig2) => {
                    if let Some(r1) = raw.find_r(*orig1) {
                        if let Some(r2) = raw.find_r(*orig2) {
                            self.roads
                                .get_mut(&r1)
                                .unwrap()
                                .override_turn_restrictions_to
                                .push(r2);
                            cnt += 1;
                        }
                    }
                }
            }
        }
        timer.note(format!("Applied {} of {} hints", cnt, hints.hints.len()));

        timer.start_iter("apply parking overrides", hints.parking_overrides.len());
        cnt = 0;
        for (orig, has_parking) in &hints.parking_overrides {
            timer.next();
            if let Some(id) = raw.find_r(*orig) {
                cnt += 1;
                self.override_parking(id, *has_parking, timer);
            }
        }
        timer.note(format!(
            "Applied {} of {} parking overrides",
            cnt,
            hints.parking_overrides.len()
        ));
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Hints {
    pub hints: Vec<Hint>,
    // Doesn't specify direction yet; all or nothing
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub parking_overrides: BTreeMap<raw_data::OriginalRoad, bool>,
}

impl Hints {
    pub fn load() -> Hints {
        if let Ok(h) = abstutil::read_json::<Hints>("../data/hints.json", &mut Timer::throwaway()) {
            h
        } else {
            Hints {
                hints: Vec::new(),
                parking_overrides: BTreeMap::new(),
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Hint {
    MergeRoad(raw_data::OriginalRoad),
    DeleteRoad(raw_data::OriginalRoad),
    MergeDegenerateIntersection(raw_data::OriginalIntersection),
    BanTurnsBetween(raw_data::OriginalRoad, raw_data::OriginalRoad),
}

pub struct LaneSpec {
    pub lane_type: LaneType,
    pub reverse_pts: bool,
}

fn get_lane_specs(
    osm_tags: &BTreeMap<String, String>,
    id: raw_data::StableRoadID,
) -> Vec<LaneSpec> {
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

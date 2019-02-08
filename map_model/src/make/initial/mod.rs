mod geometry;
pub mod lane_specs;
mod merge;

use crate::raw_data::{StableIntersectionID, StableRoadID};
use crate::{raw_data, MapEdits, LANE_THICKNESS};
use abstutil::{note, Timer};
use geom::{Bounds, Distance, GPSBounds, PolyLine, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Serialize, Deserialize)]
pub struct InitialMap {
    pub roads: BTreeMap<StableRoadID, Road>,
    pub intersections: BTreeMap<StableIntersectionID, Intersection>,

    pub name: String,
    pub bounds: Bounds,
    pub focus_on: Option<StableIntersectionID>,
    versions_saved: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Road {
    pub id: StableRoadID,
    pub src_i: StableIntersectionID,
    pub dst_i: StableIntersectionID,
    pub original_center_pts: PolyLine,
    pub trimmed_center_pts: PolyLine,
    pub fwd_width: Distance,
    pub back_width: Distance,
    pub lane_specs: Vec<lane_specs::LaneSpec>,
}

#[derive(Serialize, Deserialize)]
pub struct Intersection {
    pub id: StableIntersectionID,
    pub polygon: Vec<Pt2D>,
    pub roads: BTreeSet<StableRoadID>,
}

impl InitialMap {
    pub fn new(
        name: String,
        data: &raw_data::Map,
        gps_bounds: &GPSBounds,
        bounds: &Bounds,
        edits: &MapEdits,
        timer: &mut Timer,
    ) -> InitialMap {
        let mut m = InitialMap {
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            name,
            bounds: bounds.clone(),
            focus_on: None,
            versions_saved: 0,
        };

        for stable_id in data.intersections.keys() {
            m.intersections.insert(
                *stable_id,
                Intersection {
                    id: *stable_id,
                    polygon: Vec::new(),
                    roads: BTreeSet::new(),
                },
            );
        }

        for (stable_id, r) in &data.roads {
            if r.i1 == r.i2 {
                error!(
                    "OSM way {} is a loop on {}, skipping what would've been {}",
                    r.osm_way_id, r.i1, stable_id
                );
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

            let original_center_pts = PolyLine::new(
                r.points
                    .iter()
                    .map(|coord| Pt2D::from_gps(*coord, &gps_bounds).unwrap())
                    .collect(),
            );

            let lane_specs = lane_specs::get_lane_specs(r, *stable_id, edits);
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

            m.roads.insert(
                *stable_id,
                Road {
                    id: *stable_id,
                    src_i: r.i1,
                    dst_i: r.i2,
                    original_center_pts: original_center_pts.clone(),
                    trimmed_center_pts: original_center_pts,
                    fwd_width,
                    back_width,
                    lane_specs,
                },
            );
        }

        timer.start_iter("find each intersection polygon", m.intersections.len());
        for i in m.intersections.values_mut() {
            timer.next();

            i.polygon = geometry::intersection_polygon(i, &mut m.roads);
        }

        // TODO Move to a module if this grows.
        // Look for road center lines that hit an intersection polygon that isn't one of their
        // endpoints.
        timer.start_iter(
            "look for roads crossing intersections in strange ways",
            m.roads.len(),
        );
        for r in m.roads.values() {
            timer.next();
            // TODO Prune search.
            for i in m.intersections.values() {
                if r.src_i == i.id || r.dst_i == i.id {
                    continue;
                }
                if !r.trimmed_center_pts.crosses_polygon(&i.polygon) {
                    continue;
                }

                // TODO Avoid some false positives by seeing if this road is "close" to the
                // intersection it crosses. This probably needs more tuning. It avoids expected
                // tunnel/bridge crossings.
                if !m.floodfill(i.id, 5).contains(&r.id) {
                    continue;
                }

                // TODO Still seeing false positives due to lack of short road merging.

                note(format!("{} is suspicious -- it hits {}", r.id, i.id));
            }
        }

        if false {
            // Delete I247 and R370. remove connection of R370 from I309.
            m.intersections.remove(&StableIntersectionID(247));
            m.roads.remove(&StableRoadID(370));
            m.intersections
                .get_mut(&StableIntersectionID(309))
                .unwrap()
                .roads
                .remove(&StableRoadID(370));
            // make R253's src be the intersection it hits. recalculate that intersection.
            m.roads.get_mut(&StableRoadID(253)).unwrap().src_i = StableIntersectionID(119);
            m.intersections
                .get_mut(&StableIntersectionID(119))
                .unwrap()
                .roads
                .insert(StableRoadID(253));
            // TODO reset geometry or not?
            {
                let i = m.intersections.get_mut(&StableIntersectionID(119)).unwrap();
                i.polygon = geometry::intersection_polygon(i, &mut m.roads);
            }
            {
                // Also fix up this one.
                let i = m.intersections.get_mut(&StableIntersectionID(309)).unwrap();
                i.polygon = geometry::intersection_polygon(i, &mut m.roads);
            }
        }

        merge::short_roads(&mut m);

        m
    }

    pub fn save(&mut self, focus_on: Option<StableIntersectionID>) {
        if true {
            return;
        }
        let path = format!("../initial_maps/{:03}", self.versions_saved);
        self.focus_on = focus_on;
        self.versions_saved += 1;
        abstutil::write_binary(&path, self).expect(&format!("Saving {} failed", path));
        info!("Saved {}", path);
    }

    fn floodfill(&self, start: StableIntersectionID, steps: usize) -> HashSet<StableRoadID> {
        let mut seen: HashSet<StableRoadID> = HashSet::new();
        let mut queue: Vec<(StableRoadID, usize)> = self.intersections[&start]
            .roads
            .iter()
            .map(|r| (*r, 1))
            .collect();
        while !queue.is_empty() {
            let (r, count) = queue.pop().unwrap();
            if seen.contains(&r) {
                continue;
            }
            seen.insert(r);
            if count < steps {
                for next in self.intersections[&self.roads[&r].src_i]
                    .roads
                    .iter()
                    .chain(self.intersections[&self.roads[&r].dst_i].roads.iter())
                {
                    queue.push((*next, count + 1));
                }
            }
        }
        seen
    }
}

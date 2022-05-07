use std::collections::{BTreeMap, VecDeque};

use anyhow::Result;

use crate::{osm, IntersectionType, OriginalRoad, RawMap, RestrictionType};

// TODO After merging a road, trying to drag the surviving intersection in map_editor crashes. I
// bet the underlying problem there would help debug automated transformations near merged roads
// too.

impl RawMap {
    /// Returns (the surviving intersection, the deleted intersection, deleted roads, new roads)
    pub fn merge_short_road(
        &mut self,
        short: OriginalRoad,
    ) -> Result<(
        osm::NodeID,
        osm::NodeID,
        Vec<OriginalRoad>,
        Vec<OriginalRoad>,
    )> {
        // If either intersection attached to this road has been deleted, then we're probably
        // dealing with a short segment in the middle of a cluster of intersections. Just delete
        // the segment and move on.
        if !self.intersections.contains_key(&short.i1)
            || !self.intersections.contains_key(&short.i2)
        {
            self.roads.remove(&short).unwrap();
            bail!(
                "One endpoint of {} has already been deleted, skipping",
                short
            );
        }

        // First a sanity check.
        {
            let i1 = &self.intersections[&short.i1];
            let i2 = &self.intersections[&short.i2];
            if i1.intersection_type == IntersectionType::Border
                || i2.intersection_type == IntersectionType::Border
            {
                bail!("{} touches a border", short);
            }
        }

        // TODO Fix up turn restrictions. Many cases:
        // [ ] road we're deleting has simple restrictions
        // [ ] road we're deleting has complicated restrictions
        // [X] road we're deleting is the target of a simple BanTurns restriction
        // [ ] road we're deleting is the target of a simple OnlyAllowTurns restriction
        // [ ] road we're deleting is the target of a complicated restriction
        // [X] road we're deleting is the 'via' of a complicated restriction
        // [ ] road we're deleting has turn lanes that wind up orphaning something

        // TODO This has maybe become impossible
        let (i1, i2) = (short.i1, short.i2);
        if i1 == i2 {
            bail!("Can't merge {} -- it's a loop on {}", short, i1);
        }
        // Remember the original connections to i1 before we merge. None of these will change IDs.
        let mut connected_to_i1 = self.roads_per_intersection(i1);
        connected_to_i1.retain(|x| *x != short);

        // Retain some geometry...
        {
            let mut trim_roads_for_merging = BTreeMap::new();
            for i in [i1, i2] {
                for r in self.roads_per_intersection(i) {
                    // If we keep this in there, it might accidentally overwrite the
                    // trim_roads_for_merging key for a surviving road!
                    if r == short {
                        continue;
                    }
                    // If we're going to delete this later, don't bother!
                    if self.roads[&r].osm_tags.is("junction", "intersection") {
                        continue;
                    }

                    let pl = self.trimmed_road_geometry(r).unwrap();
                    if r.i1 == i {
                        if trim_roads_for_merging.contains_key(&(r.osm_way_id, true)) {
                            panic!("trim_roads_for_merging has an i1 duplicate for {}", r);
                        }
                        trim_roads_for_merging.insert((r.osm_way_id, true), pl.first_pt());
                    } else {
                        if trim_roads_for_merging.contains_key(&(r.osm_way_id, false)) {
                            panic!("trim_roads_for_merging has an i2 duplicate for {}", r);
                        }
                        trim_roads_for_merging.insert((r.osm_way_id, false), pl.last_pt());
                    }
                }
            }
            self.intersections
                .get_mut(&i1)
                .unwrap()
                .trim_roads_for_merging
                .extend(trim_roads_for_merging);
        }

        self.roads.remove(&short).unwrap();

        // Arbitrarily keep i1 and destroy i2. If the intersection types differ, upgrade the
        // surviving interesting.
        {
            // Don't use delete_intersection; we're manually fixing up connected roads
            let i = self.intersections.remove(&i2).unwrap();
            if i.intersection_type == IntersectionType::TrafficSignal {
                self.intersections.get_mut(&i1).unwrap().intersection_type =
                    IntersectionType::TrafficSignal;
            }
        }

        // Fix up all roads connected to i2. Delete them and create a new copy; the ID changes,
        // since one intersection changes.
        let mut deleted = vec![short];
        let mut created = Vec::new();
        let mut old_to_new = BTreeMap::new();
        let mut new_to_old = BTreeMap::new();
        for r in self.roads_per_intersection(i2) {
            deleted.push(r);
            let road = self.roads.remove(&r).unwrap();
            let mut new_id = r;
            if r.i1 == i2 {
                new_id.i1 = i1;
            } else {
                assert_eq!(r.i2, i2);
                new_id.i2 = i1;
            }

            if new_id.i1 == new_id.i2 {
                // When merging many roads around some junction, we wind up with loops. We can
                // immediately discard those.
                continue;
            }

            old_to_new.insert(r, new_id);
            new_to_old.insert(new_id, r);

            self.roads.insert(new_id, road);
            created.push(new_id);
        }

        // If we're deleting the target of a simple restriction somewhere, update it.
        for (from_id, road) in &mut self.roads {
            let mut fix_trs = Vec::new();
            for (rt, to) in road.turn_restrictions.drain(..) {
                if to == short && rt == RestrictionType::BanTurns {
                    // Remove this restriction, replace it with a new one to each of the successors
                    // of the deleted road. Depending if the intersection we kept is the one
                    // connecting these two roads, the successors differ.
                    if new_to_old
                        .get(from_id)
                        .cloned()
                        .unwrap_or(*from_id)
                        .common_endpt(short)
                        == i1
                    {
                        for x in &created {
                            fix_trs.push((rt, *x));
                        }
                    } else {
                        for x in &connected_to_i1 {
                            fix_trs.push((rt, *x));
                        }
                    }
                } else {
                    fix_trs.push((rt, to));
                }
            }
            road.turn_restrictions = fix_trs;
        }

        // If we're deleting the 'via' of a complicated restriction somewhere, change it to a
        // simple restriction.
        for road in self.roads.values_mut() {
            let mut add = Vec::new();
            road.complicated_turn_restrictions.retain(|(via, to)| {
                if *via == short {
                    // Depending which intersection we're deleting, the ID of 'to' might change
                    let to_id = old_to_new.get(to).cloned().unwrap_or(*to);
                    add.push((RestrictionType::BanTurns, to_id));
                    false
                } else {
                    true
                }
            });
            road.turn_restrictions.extend(add);
        }

        Ok((i1, i2, deleted, created))
    }
}

/// Merge all roads marked with `junction=intersection`
pub fn merge_all_junctions(map: &mut RawMap) {
    let mut queue: VecDeque<OriginalRoad> = VecDeque::new();
    for (id, road) in &map.roads {
        if road.osm_tags.is("junction", "intersection") {
            queue.push_back(*id);
        }
    }

    while !queue.is_empty() {
        let id = queue.pop_front().unwrap();

        // The road might've been deleted by a previous merge_short_road call
        if !map.roads.contains_key(&id) {
            continue;
        }

        match map.merge_short_road(id) {
            Ok((_, _, _, new_roads)) => {
                // Some road IDs still in the queue might have changed, so check the new_roads for
                // anything we should try to merge
                for r in new_roads {
                    if map.roads[&r].osm_tags.is("junction", "intersection") {
                        queue.push_back(r);
                    }
                }
            }
            Err(err) => {
                warn!("Not merging short road / junction=intersection: {}", err);
            }
        }
    }
}

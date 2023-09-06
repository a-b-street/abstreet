use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use geom::{Distance, Line};

use crate::{EditCmd, IntersectionID, Map, PathConstraints, RoadID};

/// The type of a modal filter. Most of these don't have semantics yet; the variation is just for
/// visual representation
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FilterType {
    NoEntry,
    WalkCycleOnly,
    BusGate,
    SchoolStreet,
}

/// A filter placed somewhere along a road
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RoadFilter {
    pub dist: Distance,
    pub filter_type: FilterType,
}

impl RoadFilter {
    pub fn new(dist: Distance, filter_type: FilterType) -> Self {
        Self { dist, filter_type }
    }
}

/// A diagonal filter exists in an intersection. It's defined by two roads (the order is
/// arbitrary). When all of the intersection's roads are sorted in clockwise order, this pair of
/// roads splits the ordering into two groups. Turns in each group are still possible, but not
/// across groups.
///
/// Be careful with `PartialEq` -- see `approx_eq`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct DiagonalFilter {
    pub i: IntersectionID,
    pub r1: RoadID,
    pub r2: RoadID,
    pub filter_type: FilterType,

    pub group1: BTreeSet<RoadID>,
    pub group2: BTreeSet<RoadID>,
}

impl DiagonalFilter {
    /// The caller must call this in a `before_edit` / `redraw_all_icons` "transaction."
    pub fn cycle_through_alternatives(
        map: &Map,
        i: IntersectionID,
        filter_type: FilterType,
    ) -> Vec<EditCmd> {
        let mut roads = map.get_i(i).roads.clone();
        // Don't consider non-driveable roads for the 4-way calculation even
        // TODO This should check access=no,private too
        roads.retain(|r| PathConstraints::Car.can_use_road(map.get_r(*r), map));

        let mut commands = Vec::new();

        if roads.len() == 4 {
            // 4-way intersections are the only place where true diagonal filters can be placed
            let alt1 = DiagonalFilter::new(map, i, roads[0], roads[1], filter_type);
            let alt2 = DiagonalFilter::new(map, i, roads[1], roads[2], filter_type);

            match map.get_i(i).modal_filter {
                Some(ref prev) => {
                    if alt1.approx_eq(prev) {
                        commands.push(map.edit_intersection_cmd(i, |new| {
                            new.modal_filter = Some(alt2);
                        }));
                    } else if alt2.approx_eq(prev) {
                        commands.push(map.edit_intersection_cmd(i, |new| {
                            new.modal_filter = None;
                        }));
                    } else {
                        unreachable!()
                    }
                }
                None => {
                    commands.push(map.edit_intersection_cmd(i, |new| {
                        new.modal_filter = Some(alt1);
                    }));
                }
            }
        } else if roads.len() > 1 {
            // Diagonal filters elsewhere don't really make sense. They're equivalent to filtering
            // one road. Just cycle through those.

            // But skip roads that're aren't filterable
            roads.retain(|r| {
                let road = map.get_r(*r);
                road.oneway_for_driving().is_none() && !road.is_deadend_for_driving(map)
            });

            // TODO I triggered this case somewhere in Kennington when drawing free-hand. Look for
            // the case and test this case more carefully. Maybe do the filtering earlier.
            if roads.is_empty() {
                return commands;
            }

            let mut add_filter_to = None;
            if let Some(idx) = roads
                .iter()
                .position(|r| map.get_r(*r).modal_filter.is_some())
            {
                commands.push(map.edit_road_cmd(roads[idx], |new| {
                    new.modal_filter = None;
                }));
                if idx != roads.len() - 1 {
                    add_filter_to = Some(roads[idx + 1]);
                }
            } else {
                add_filter_to = Some(roads[0]);
            }
            if let Some(r) = add_filter_to {
                let road = map.get_r(r);
                let dist = if i == road.src_i {
                    Distance::ZERO
                } else {
                    road.length()
                };
                commands.push(map.edit_road_cmd(r, |new| {
                    new.modal_filter = Some(RoadFilter::new(dist, filter_type));
                }));
            }
        }
        commands
    }

    fn new(
        map: &Map,
        i: IntersectionID,
        r1: RoadID,
        r2: RoadID,
        filter_type: FilterType,
    ) -> DiagonalFilter {
        let mut roads = map.get_i(i).roads.clone();
        // Make self.r1 be the first entry
        while roads[0] != r1 {
            roads.rotate_right(1);
        }

        let mut group1 = BTreeSet::new();
        group1.insert(roads.remove(0));
        loop {
            let next = roads.remove(0);
            group1.insert(next);
            if next == r2 {
                break;
            }
        }
        // This is only true for 4-ways...
        assert_eq!(group1.len(), 2);
        assert_eq!(roads.len(), 2);

        DiagonalFilter {
            r1,
            r2,
            i,
            filter_type,
            group1,
            group2: roads.into_iter().collect(),
        }
    }

    /// Physically where is the filter placed?
    pub fn geometry(&self, map: &Map) -> Line {
        let r1 = map.get_r(self.r1);
        let r2 = map.get_r(self.r2);

        // Orient the road to face the intersection
        let pl1 = r1.center_pts.maybe_reverse(r1.src_i == self.i);
        let pl2 = r2.center_pts.maybe_reverse(r2.src_i == self.i);

        // The other combinations of left/right here would produce points or a line across just one
        // road
        let pt1 = pl1.must_shift_right(r1.get_half_width()).last_pt();
        let pt2 = pl2.must_shift_left(r2.get_half_width()).last_pt();
        match Line::new(pt1, pt2) {
            Ok(line) => line,
            // Very rarely, this line is too small. If that happens, just draw something roughly in
            // the right place
            Err(_) => Line::must_new(
                pt1,
                pt1.project_away(r1.get_half_width(), pt1.angle_to(pt2)),
            ),
        }
    }

    pub fn allows_turn(&self, from: RoadID, to: RoadID) -> bool {
        self.group1.contains(&from) == self.group1.contains(&to)
    }

    pub fn avoid_movements_between_roads(&self) -> Vec<(RoadID, RoadID)> {
        let mut pairs = Vec::new();
        for from in &self.group1 {
            for to in &self.group2 {
                pairs.push((*from, *to));
                pairs.push((*to, *from));
            }
        }
        pairs
    }

    fn approx_eq(&self, other: &DiagonalFilter) -> bool {
        // Careful. At a 4-way intersection, the same filter can be expressed as a different pair of two
        // roads. The (r1, r2) ordering is also arbitrary. cycle_through_alternatives is
        // consistent, though.
        //
        // Note this ignores filter_type.
        (self.r1, self.r2, self.i, &self.group1, &self.group2)
            == (other.r1, other.r2, other.i, &other.group1, &other.group2)
    }
}

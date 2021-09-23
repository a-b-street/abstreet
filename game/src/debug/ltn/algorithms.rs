use std::collections::{BTreeSet, HashMap};

use map_model::osm::RoadRank;
use map_model::{IntersectionID, Map, PathConstraints, RoadID};

use crate::debug::ltn::{Neighborhood, RatRun};

impl Neighborhood {
    // TODO Doesn't find the full perimeter. But do we really need that?
    pub fn from_road(map: &Map, start: RoadID) -> Neighborhood {
        // Do a simple floodfill from this road, stopping anytime we find a major road
        let mut interior = BTreeSet::new();
        let mut perimeter = BTreeSet::new();
        let mut borders = BTreeSet::new();

        // We don't need a priority queue
        let mut visited = BTreeSet::new();
        let mut queue = vec![start];
        interior.insert(start);

        while !queue.is_empty() {
            let current = map.get_r(queue.pop().unwrap());
            if visited.contains(&current.id) {
                continue;
            }
            visited.insert(current.id);
            for i in [current.src_i, current.dst_i] {
                let (minor, major): (Vec<&RoadID>, Vec<&RoadID>) =
                    map.get_i(i).roads.iter().partition(|r| {
                        let road = map.get_r(**r);
                        road.get_rank() == RoadRank::Local
                            && road
                                .lanes
                                .iter()
                                .any(|l| PathConstraints::Car.can_use(l, map))
                    });
                if major.is_empty() {
                    for r in minor {
                        interior.insert(*r);
                        queue.push(*r);
                    }
                } else {
                    borders.insert(i);
                    perimeter.extend(major);
                }
            }
        }

        Neighborhood {
            interior,
            perimeter,
            borders,
        }
    }

    // Just returns a sampling of rat runs, not necessarily all of them
    pub fn find_rat_runs(&self, map: &Map) -> Vec<RatRun> {
        // Just flood from each border and see if we can reach another border.
        //
        // We might be able to do this in one pass, seeding the queue with all borders. But I think
        // the "visited" bit would get tangled up between different possibilities...
        self.borders
            .iter()
            .flat_map(|i| self.rat_run_from(map, *i))
            .collect()
    }

    fn rat_run_from(&self, map: &Map, start: IntersectionID) -> Option<RatRun> {
        // We don't need a priority queue
        let mut back_refs = HashMap::new();
        let mut queue = vec![start];

        while !queue.is_empty() {
            let current = queue.pop().unwrap();
            if current != start && self.borders.contains(&current) {
                // Found one!
                let mut at = current;
                let mut path = vec![at];
                while let Some(prev) = back_refs.remove(&at) {
                    path.push(prev);
                    at = prev;
                }
                path.push(start);
                path.reverse();
                return Some(RatRun { path });
            }

            for r in &map.get_i(current).roads {
                let next = map.get_r(*r).other_endpt(current);
                if !self.interior.contains(r) || back_refs.contains_key(&next) {
                    continue;
                }
                back_refs.insert(next, current);
                queue.push(next);
            }
        }

        None
    }
}

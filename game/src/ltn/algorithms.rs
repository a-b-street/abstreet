use std::collections::{BTreeSet, HashMap};

use geom::Distance;
use map_model::osm::RoadRank;
use map_model::{IntersectionID, Map, PathConstraints, RoadID};

use crate::ltn::{Neighborhood, RatRun};

impl Neighborhood {
    // TODO Doesn't find the full perimeter. But do we really need that?
    pub fn from_road(map: &Map, start: RoadID) -> Neighborhood {
        assert!(Neighborhood::is_interior_road(start, map));

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
                let (minor, major): (Vec<&RoadID>, Vec<&RoadID>) = map
                    .get_i(i)
                    .roads
                    .iter()
                    .partition(|r| Neighborhood::is_interior_road(**r, map));
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

        let mut n = Neighborhood {
            interior,
            perimeter,
            borders,

            modal_filters: BTreeSet::new(),
            rat_runs: Vec::new(),
        };
        n.rat_runs = n.find_rat_runs(map);
        n
    }

    pub fn toggle_modal_filter(&mut self, map: &Map, r: RoadID) {
        if self.modal_filters.contains(&r) {
            self.modal_filters.remove(&r);
        } else {
            self.modal_filters.insert(r);
        }
        self.rat_runs = self.find_rat_runs(map);
    }

    pub fn is_interior_road(r: RoadID, map: &Map) -> bool {
        let road = map.get_r(r);
        road.get_rank() == RoadRank::Local
            && road
                .lanes
                .iter()
                .any(|l| PathConstraints::Car.can_use(l, map))
    }

    // Just returns a sampling of rat runs, not necessarily all of them
    fn find_rat_runs(&self, map: &Map) -> Vec<RatRun> {
        // Just flood from each border and see if we can reach another border.
        //
        // We might be able to do this in one pass, seeding the queue with all borders. But I think
        // the "visited" bit would get tangled up between different possibilities...
        let mut runs: Vec<RatRun> = self
            .borders
            .iter()
            .flat_map(|i| self.rat_run_from(map, *i))
            .collect();
        runs.sort_by(|a, b| a.length_ratio.partial_cmp(&b.length_ratio).unwrap());
        runs
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
                return Some(RatRun::new(path, map));
            }

            for r in &map.get_i(current).roads {
                let next = map.get_r(*r).other_endpt(current);
                if !self.interior.contains(r)
                    || self.modal_filters.contains(r)
                    || back_refs.contains_key(&next)
                {
                    continue;
                }
                back_refs.insert(next, current);
                queue.push(next);
            }
        }

        None
    }
}

impl RatRun {
    fn new(path: Vec<IntersectionID>, map: &Map) -> RatRun {
        let mut run = RatRun {
            path,
            length_ratio: 1.0,
        };
        if let Some((roads, _)) = map.simple_path_btwn(run.path[0], *run.path.last().unwrap()) {
            let shortest: Distance = roads
                .into_iter()
                .map(|r| map.get_r(r).center_pts.length())
                .sum();
            let this_path: Distance = run.roads(map).map(|r| r.center_pts.length()).sum();
            run.length_ratio = this_path / shortest;
        }
        run
    }
}

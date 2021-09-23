use std::collections::BTreeSet;

use map_model::osm::RoadRank;
use map_model::{Map, PathConstraints, RoadID};

use crate::debug::ltn::Neighborhood;

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
}

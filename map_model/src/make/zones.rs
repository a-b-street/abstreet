use crate::{Map, RoadID, Zone, ZoneID};
use std::collections::BTreeSet;

pub fn make_all_zones(map: &Map) -> Vec<Zone> {
    let mut queue = Vec::new();
    for r in map.all_roads() {
        if r.osm_tags.get("access") == Some(&"private".to_string()) {
            queue.push(r.id);
        }
    }

    let mut zones = Vec::new();
    let mut seen = BTreeSet::new();
    while !queue.is_empty() {
        let start = queue.pop().unwrap();
        if seen.contains(&start) {
            continue;
        }
        let zone = floodfill(map, start, ZoneID(zones.len()));
        seen.extend(zone.members.clone());
        zones.push(zone);
    }

    zones
}

fn floodfill(map: &Map, start: RoadID, id: ZoneID) -> Zone {
    let mut queue = vec![start];
    let mut members = BTreeSet::new();
    let mut borders = BTreeSet::new();
    while !queue.is_empty() {
        let current = queue.pop().unwrap();
        if members.contains(&current) {
            continue;
        }
        members.insert(current);
        for r in map.get_next_roads(current) {
            let r = map.get_r(r);
            if r.osm_tags.get("access") == Some(&"private".to_string()) {
                queue.push(r.id);
            } else {
                borders.insert(map.get_r(current).common_endpt(r));
            }
        }
    }
    assert!(!members.is_empty());
    assert!(!borders.is_empty());
    Zone {
        id,
        members,
        borders,
        allow_through_traffic: BTreeSet::new(),
    }
}

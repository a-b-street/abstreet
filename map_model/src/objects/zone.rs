//! Zones and AccessRestrictions are used to model things like:
//! 1) gated communities, where only trips beginning or ending at a building in the neighborhood may
//!    use any of the private roads
//! 2) Stay Healthy Streets, where most car traffic is banned, except for trips beginning/ending in
//!    the zone
//! 3) Congestion capping, where only so many cars per hour can enter the zone

use std::collections::BTreeSet;

use enumset::EnumSet;
use serde::{Deserialize, Serialize};

use crate::{CommonEndpoint, IntersectionID, Map, PathConstraints, RoadID};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AccessRestrictions {
    pub allow_through_traffic: EnumSet<PathConstraints>,
}

impl AccessRestrictions {
    pub fn new() -> AccessRestrictions {
        AccessRestrictions {
            allow_through_traffic: EnumSet::all(),
        }
    }
}

/// A contiguous set of roads with access restrictions. This is derived from all the map's roads and
/// kept cached for performance.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Zone {
    pub members: BTreeSet<RoadID>,
    pub borders: BTreeSet<IntersectionID>,
    pub restrictions: AccessRestrictions,
}

impl Zone {
    pub fn make_all(map: &Map) -> Vec<Zone> {
        let mut queue = Vec::new();
        for r in map.all_roads() {
            if r.is_private() {
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
            if let Some(zone) = floodfill(map, start) {
                seen.extend(zone.members.clone());
                zones.push(zone);
            }
        }

        zones
    }
}

fn floodfill(map: &Map, start: RoadID) -> Option<Zone> {
    let match_constraints = map.get_r(start).access_restrictions.clone();
    let merge_zones = map.get_edits().merge_zones;
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
            if r.access_restrictions == match_constraints && merge_zones {
                queue.push(r.id);
            } else {
                // TODO Handle other cases
                if let CommonEndpoint::One(i) = map.get_r(current).common_endpoint(r) {
                    borders.insert(i);
                }
            }
        }
    }
    assert!(!members.is_empty());
    if borders.is_empty() {
        // This can happen around cases like https://www.openstreetmap.org/way/572240785 where the
        // intersection geometry seems to break
        return None;
    }
    Some(Zone {
        members,
        borders,
        restrictions: match_constraints,
    })
}

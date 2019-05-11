use crate::{raw_data, IntersectionID, LaneID, LaneType, LANE_THICKNESS};
use abstutil::{Error, Warn};
use geom::{Distance, PolyLine, Polygon, Speed};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoadID(pub usize);

impl fmt::Display for RoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RoadID({0})", self.0)
    }
}

impl RoadID {
    pub fn forwards(self) -> DirectedRoadID {
        DirectedRoadID {
            id: self,
            forwards: true,
        }
    }

    pub fn backwards(self) -> DirectedRoadID {
        DirectedRoadID {
            id: self,
            forwards: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DirectedRoadID {
    pub id: RoadID,
    pub forwards: bool,
}

impl fmt::Display for DirectedRoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DirectedRoadID({}, {})",
            self.id.0,
            if self.forwards {
                "forwards"
            } else {
                "backwards"
            }
        )
    }
}

// These're bidirectional (possibly)
#[derive(Serialize, Deserialize, Debug)]
pub struct Road {
    pub id: RoadID,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
    pub stable_id: raw_data::StableRoadID,

    // Invariant: A road must contain at least one child
    // These are ordered from left-most lane (closest to center lane) to rightmost (sidewalk)
    pub children_forwards: Vec<(LaneID, LaneType)>,
    pub children_backwards: Vec<(LaneID, LaneType)>,
    // TODO should consider having a redundant lookup from LaneID

    // Unshifted center points. Order implies road orientation.
    pub center_pts: PolyLine,
    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,

    // For debugging.
    pub original_center_pts: PolyLine,

    // Need to retain for map editing.
    pub parking_lane_fwd: bool,
    pub parking_lane_back: bool,
}

impl Road {
    pub fn get_lane_types(&self) -> (Vec<LaneType>, Vec<LaneType>) {
        (
            self.children_forwards.iter().map(|pair| pair.1).collect(),
            self.children_backwards.iter().map(|pair| pair.1).collect(),
        )
    }

    pub fn is_forwards(&self, lane: LaneID) -> bool {
        self.dir_and_offset(lane).0
    }

    pub fn is_backwards(&self, lane: LaneID) -> bool {
        !self.dir_and_offset(lane).0
    }

    // lane must belong to this road. Offset 0 is the centermost lane on each side of a road, then
    // it counts up from there. Returns true for the forwards direction, false for backwards.
    pub fn dir_and_offset(&self, lane: LaneID) -> (bool, usize) {
        if let Some(idx) = self
            .children_forwards
            .iter()
            .position(|pair| pair.0 == lane)
        {
            return (true, idx);
        }
        if let Some(idx) = self
            .children_backwards
            .iter()
            .position(|pair| pair.0 == lane)
        {
            return (false, idx);
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    pub fn parking_to_driving(&self, parking: LaneID) -> Option<LaneID> {
        // TODO Crossing bike/bus lanes means higher layers of sim should know to block these off
        // when parking/unparking
        let (fwds, idx) = self.dir_and_offset(parking);
        if fwds {
            self.children_forwards[0..idx]
                .iter()
                .rev()
                .find(|(_, lt)| *lt == LaneType::Driving)
                .map(|(id, _)| *id)
        } else {
            self.children_backwards[0..idx]
                .iter()
                .rev()
                .find(|(_, lt)| *lt == LaneType::Driving)
                .map(|(id, _)| *id)
        }
    }

    pub fn sidewalk_to_bike(&self, sidewalk: LaneID) -> Option<LaneID> {
        // TODO Crossing bus lanes means higher layers of sim should know to block these off
        let (fwds, idx) = self.dir_and_offset(sidewalk);
        if fwds {
            self.children_forwards[0..idx]
                .iter()
                .rev()
                .find(|(_, lt)| *lt == LaneType::Driving || *lt == LaneType::Biking)
                .map(|(id, _)| *id)
        } else {
            self.children_backwards[0..idx]
                .iter()
                .rev()
                .find(|(_, lt)| *lt == LaneType::Driving || *lt == LaneType::Biking)
                .map(|(id, _)| *id)
        }
    }

    pub fn bike_to_sidewalk(&self, bike: LaneID) -> Option<LaneID> {
        // TODO Crossing bus lanes means higher layers of sim should know to block these off
        let (fwds, idx) = self.dir_and_offset(bike);
        if fwds {
            self.children_forwards[idx..]
                .iter()
                .find(|(_, lt)| *lt == LaneType::Sidewalk)
                .map(|(id, _)| *id)
        } else {
            self.children_backwards[idx..]
                .iter()
                .find(|(_, lt)| *lt == LaneType::Sidewalk)
                .map(|(id, _)| *id)
        }
    }

    // Is this lane the arbitrary canonical lane of this road? Used for deciding who should draw
    // yellow center lines.
    pub fn is_canonical_lane(&self, lane: LaneID) -> bool {
        if !self.children_forwards.is_empty() {
            return lane == self.children_forwards[0].0;
        }
        lane == self.children_backwards[0].0
    }

    pub fn get_speed_limit(&self) -> Speed {
        // TODO Should probably cache this
        if let Some(limit) = self.osm_tags.get("maxspeed") {
            // TODO handle other units
            if limit.ends_with(" mph") {
                if let Ok(mph) = limit[0..limit.len() - 4].parse::<f64>() {
                    return Speed::miles_per_hour(mph);
                }
            }
        }

        if self.osm_tags.get("highway") == Some(&"primary".to_string())
            || self.osm_tags.get("highway") == Some(&"secondary".to_string())
        {
            return Speed::miles_per_hour(40.0);
        }
        Speed::miles_per_hour(20.0)
    }

    pub fn get_zorder(&self) -> isize {
        // TODO Should probably cache this
        if let Some(layer) = self.osm_tags.get("layer") {
            layer.parse::<isize>().unwrap()
        } else {
            0
        }
    }

    pub fn incoming_lanes(&self, i: IntersectionID) -> &Vec<(LaneID, LaneType)> {
        if self.src_i == i {
            &self.children_backwards
        } else if self.dst_i == i {
            &self.children_forwards
        } else {
            panic!("{} doesn't have an endpoint at {}", self.id, i);
        }
    }

    pub fn outgoing_lanes(&self, i: IntersectionID) -> &Vec<(LaneID, LaneType)> {
        if self.src_i == i {
            &self.children_forwards
        } else if self.dst_i == i {
            &self.children_backwards
        } else {
            panic!("{} doesn't have an endpoint at {}", self.id, i);
        }
    }

    // If 'from' is a sidewalk, we'll also consider lanes on the other side of the road, if needed.
    // TODO But reusing dist_along will break loudly in that case! Really need a perpendicular
    // projection-and-collision method to find equivalent dist_along's.
    pub(crate) fn find_closest_lane(
        &self,
        from: LaneID,
        types: Vec<LaneType>,
    ) -> Result<LaneID, Error> {
        let lane_types: HashSet<LaneType> = types.into_iter().collect();
        let (dir, from_idx) = self.dir_and_offset(from);
        let mut list = if dir {
            &self.children_forwards
        } else {
            &self.children_backwards
        };
        // Deal with one-ways and sidewalks on both sides
        if list.len() == 1 && list[0].1 == LaneType::Sidewalk {
            list = if dir {
                &self.children_backwards
            } else {
                &self.children_forwards
            };
        }

        if let Some((_, lane)) = list
            .iter()
            .enumerate()
            .filter(|(_, (lane, lt))| *lane != from && lane_types.contains(lt))
            .map(|(idx, (lane, _))| (((from_idx as isize) - (idx as isize)).abs(), *lane))
            .min_by_key(|(offset, _)| *offset)
        {
            Ok(lane)
        } else {
            Err(Error::new(format!(
                "{} isn't near a {:?} lane",
                from, lane_types
            )))
        }
    }

    pub fn supports_bikes(&self) -> bool {
        // TODO Should check LaneType to start
        self.osm_tags.get("bicycle") != Some(&"no".to_string())
    }

    pub fn all_lanes(&self) -> Vec<LaneID> {
        self.children_forwards
            .iter()
            .map(|(id, _)| *id)
            .chain(self.children_backwards.iter().map(|(id, _)| *id))
            .collect()
    }

    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }

    pub fn any_on_other_side(&self, l: LaneID, lt: LaneType) -> Option<LaneID> {
        let search = if self.is_forwards(l) {
            &self.children_backwards
        } else {
            &self.children_forwards
        };
        search.iter().find(|(_, t)| lt == *t).map(|(id, _)| *id)
    }

    pub fn get_thick_polyline(&self) -> Warn<(PolyLine, Distance)> {
        let width_right = (self.children_forwards.len() as f64) * LANE_THICKNESS;
        let width_left = (self.children_backwards.len() as f64) * LANE_THICKNESS;
        let total_width = width_right + width_left;
        if width_right >= width_left {
            self.center_pts
                .shift_right((width_right - width_left) / 2.0)
                .map(|pl| (pl, total_width))
        } else {
            self.center_pts
                .shift_left((width_left - width_right) / 2.0)
                .map(|pl| (pl, total_width))
        }
    }

    pub fn get_thick_polygon(&self) -> Warn<Polygon> {
        self.get_thick_polyline()
            .map(|(pl, width)| pl.make_polygons(width))
    }

    // Also returns width. The polyline points the correct direction. None if no lanes that
    // direction.
    pub fn get_center_for_side(&self, fwds: bool) -> Option<Warn<(PolyLine, Distance)>> {
        if fwds {
            if self.children_forwards.is_empty() {
                return None;
            }
            let width = LANE_THICKNESS * (self.children_forwards.len() as f64);
            Some(
                self.center_pts
                    .shift_right(width / 2.0)
                    .map(|pl| (pl, width)),
            )
        } else {
            if self.children_backwards.is_empty() {
                return None;
            }
            let width = LANE_THICKNESS * (self.children_backwards.len() as f64);
            Some(
                self.center_pts
                    .shift_left(width / 2.0)
                    .map(|pl| (pl.reversed(), width)),
            )
        }
    }

    pub fn get_name(&self) -> String {
        if let Some(name) = self.osm_tags.get("name") {
            return name.to_string();
        }
        if let Some(name) = self.osm_tags.get("ref") {
            return name.to_string();
        }
        if self
            .osm_tags
            .get("highway")
            .map(|hwy| hwy.ends_with("_link"))
            .unwrap_or(false)
        {
            if let Some(name) = self.osm_tags.get("destination:street") {
                return format!("Exit for {}", name);
            }
            if let Some(name) = self.osm_tags.get("destination:ref") {
                return format!("Exit for {}", name);
            }
            if let Some(name) = self.osm_tags.get("destination") {
                return format!("Exit for {}", name);
            }
            // Sometimes 'directions' is filled out, but incorrectly...
        }
        "???".to_string()
    }
}

use crate::raw::{OriginalRoad, RestrictionType};
use crate::{osm, BusStopID, IntersectionID, LaneID, LaneType, Map, PathConstraints};
use abstutil::{Error, Warn};
use geom::{Distance, PolyLine, Polygon, Speed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoadID(pub usize);

impl fmt::Display for RoadID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Road #{}", self.0)
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

impl DirectedRoadID {
    pub fn src_i(self, map: &Map) -> IntersectionID {
        let r = map.get_r(self.id);
        if self.forwards {
            r.src_i
        } else {
            r.dst_i
        }
    }

    pub fn dst_i(self, map: &Map) -> IntersectionID {
        let r = map.get_r(self.id);
        if self.forwards {
            r.dst_i
        } else {
            r.src_i
        }
    }

    // Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub fn lanes(self, constraints: PathConstraints, map: &Map) -> Vec<LaneID> {
        let r = map.get_r(self.id);
        constraints.filter_lanes(r.children(self.forwards).iter().map(|(l, _)| *l), map)
    }
}

// These're bidirectional (possibly)
#[derive(Serialize, Deserialize, Debug)]
pub struct Road {
    pub id: RoadID,
    // I've previously tried storing these in a compressed lookup table (since the keys and values
    // are often common), but the performance benefit was negligible, and the increased API
    // complexity was annoying.
    pub osm_tags: BTreeMap<String, String>,
    // self is 'from'
    pub turn_restrictions: Vec<(RestrictionType, RoadID)>,
    // self is 'from'. (via, to). Only BanTurns.
    pub complicated_turn_restrictions: Vec<(RoadID, RoadID)>,
    pub orig_id: OriginalRoad,
    pub speed_limit: Speed,
    pub zorder: isize,

    // Invariant: A road must contain at least one child
    // These are ordered from closest to center lane (left-most when driving on the right) to
    // farthest (sidewalk)
    pub children_forwards: Vec<(LaneID, LaneType)>,
    pub children_backwards: Vec<(LaneID, LaneType)>,

    // Unshifted original center points. Order implies road orientation. Reversing lanes doesn't
    // change this.
    pub center_pts: PolyLine,
    pub src_i: IntersectionID,
    pub dst_i: IntersectionID,
}

type HomogenousTuple2<T> = (T, T);

impl Road {
    pub fn children(&self, fwds: bool) -> &Vec<(LaneID, LaneType)> {
        if fwds {
            &self.children_forwards
        } else {
            &self.children_backwards
        }
    }

    pub fn children_mut(&mut self, fwds: bool) -> &mut Vec<(LaneID, LaneType)> {
        if fwds {
            &mut self.children_forwards
        } else {
            &mut self.children_backwards
        }
    }

    pub fn get_lane_types<'slf>(&'slf self) -> HomogenousTuple2<impl Iterator<Item=LaneType> + 'slf> {
        let get_lanetype = |(_, lanetype): &(_, LaneType)| *lanetype;
        (
            self.children(true).iter().map(get_lanetype.clone()),
            self.children_backwards.iter().map(get_lanetype.clone()),
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
        for &fwds in [true, false].iter() {
            if let Some(idx) = self
                .children(fwds)
                .iter()
                .position(|pair| pair.0 == lane)
            {
                return (fwds, idx);
            }
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    pub fn parking_to_driving(&self, parking: LaneID) -> Option<LaneID> {
        // TODO Crossing bike/bus lanes means higher layers of sim should know to block these off
        // when parking/unparking
        let (fwds, idx) = self.dir_and_offset(parking);
        self.children(fwds)[0..idx]
            .iter()
            .rev()
            .chain(self.children(!fwds).iter())
            .find(|(_, lt)| *lt == LaneType::Driving)
            .map(|(id, _)| *id)
    }

    pub fn sidewalk_to_bike(&self, sidewalk: LaneID) -> Option<LaneID> {
        // TODO Crossing bus lanes means higher layers of sim should know to block these off
        // Oneways mean we might need to consider the other side of the road.
        let (fwds, idx) = self.dir_and_offset(sidewalk);
        self.children(fwds)[0..idx]
            .iter()
            .rev()
            .chain(self.children(!fwds).iter())
            .find(|(_, lt)| *lt == LaneType::Driving || *lt == LaneType::Biking)
            .map(|(id, _)| *id)
    }

    pub fn bike_to_sidewalk(&self, bike: LaneID) -> Option<LaneID> {
        // TODO Crossing bus lanes means higher layers of sim should know to block these off
        let (fwds, idx) = self.dir_and_offset(bike);
        self.children(fwds)[idx..]
            .iter()
            .find(|(_, lt)| *lt == LaneType::Sidewalk)
            .map(|(id, _)| *id)
    }

    pub(crate) fn speed_limit_from_osm(&self) -> Speed {
        if let Some(limit) = self.osm_tags.get(osm::MAXSPEED) {
            // TODO handle other units
            if limit.ends_with(" mph") {
                if let Ok(mph) = limit[0..limit.len() - 4].parse::<f64>() {
                    return Speed::miles_per_hour(mph);
                }
            }
        }

        if self.osm_tags.get(osm::HIGHWAY) == Some(&"primary".to_string())
            || self.osm_tags.get(osm::HIGHWAY) == Some(&"secondary".to_string())
        {
            return Speed::miles_per_hour(40.0);
        }
        Speed::miles_per_hour(20.0)
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
        let mut list = self.children(dir);
        // Deal with one-ways and sidewalks on both sides
        if list.len() == 1 && list[0].1 == LaneType::Sidewalk {
            list = self.children(!dir);
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

    pub fn all_lanes(&self) -> Vec<LaneID> {
        self.children_forwards
            .iter()
            .chain(self.children_backwards.iter())
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn lanes_on_side<'slf>(&'slf self, dir: bool) -> impl Iterator<Item=LaneID> + 'slf {
        self.children(dir)
            .iter()
            .map(|(id, _)| *id)
    }

    pub fn get_current_center(&self, map: &Map) -> PolyLine {
        // The original center_pts don't account for contraflow lane edits.
        let lane = map.get_l(if !self.children_forwards.is_empty() {
            self.children_forwards[0].0
        } else {
            self.children_backwards[0].0
        });
        map.left_shift(lane.lane_center_pts.clone(), lane.width / 2.0)
            .unwrap()
    }

    pub fn any_on_other_side(&self, l: LaneID, lt: LaneType) -> Option<LaneID> {
        let search = self.children(!self.is_forwards(l));
        search.iter().find(|(_, t)| lt == *t).map(|(id, _)| *id)
    }

    pub fn width(&self, map: &Map, fwds: bool) -> Distance {
        self.children(fwds)
            .iter()
            .map(|(l, _)| map.get_l(*l).width)
            .sum()
    }
    pub fn width_fwd(&self, map: &Map) -> Distance {
        self.width(map, true)
    }
    pub fn width_back(&self, map: &Map) -> Distance {
        self.width(map, false)
    }

    pub fn get_thick_polyline(&self, map: &Map) -> Warn<(PolyLine, Distance)> {
        let fwd = self.width_fwd(map);
        let back = self.width_back(map);

        if fwd >= back {
            map.right_shift(self.center_pts.clone(), (fwd - back) / 2.0)
                .map(|pl| (pl, fwd + back))
        } else {
            map.left_shift(self.center_pts.clone(), (back - fwd) / 2.0)
                .map(|pl| (pl, fwd + back))
        }
    }

    pub fn get_thick_polygon(&self, map: &Map) -> Warn<Polygon> {
        self.get_thick_polyline(map)
            .map(|(pl, width)| pl.make_polygons(width))
    }

    pub fn get_name(&self) -> String {
        if let Some(name) = self.osm_tags.get(osm::NAME) {
            if name == "" {
                return "???".to_string();
            } else {
                return name.to_string();
            }
        }
        if let Some(name) = self.osm_tags.get("ref") {
            return name.to_string();
        }
        if self
            .osm_tags
            .get(osm::HIGHWAY)
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

    // Used to determine which roads have stop signs when roads of different types intersect.
    pub fn get_rank(&self) -> usize {
        if let Some(highway) = self.osm_tags.get(osm::HIGHWAY) {
            match highway.as_ref() {
                "motorway" => 20,
                "motorway_link" => 19,
                // TODO Probably not true in general. For the West Seattle bridge.
                "construction" => 18,

                "trunk" => 17,
                "trunk_link" => 16,

                "primary" => 15,
                "primary_link" => 14,

                "secondary" => 13,
                "secondary_link" => 12,

                "tertiary" => 10,
                "tertiary_link" => 9,

                "residential" => 5,

                "footway" => 1,

                "unclassified" => 0,
                "road" => 0,
                "crossing" => 0,
                // If you hit this error and the highway type doesn't represent a driveable road,
                // you may want to instead filter out the OSM way entirely in
                // convert_osm/src/osm_reader.rs's is_road().
                _ => panic!("Unknown OSM highway {}", highway),
            }
        } else {
            0
        }
    }

    pub fn all_bus_stops(&self, map: &Map) -> Vec<BusStopID> {
        let mut stops = Vec::new();
        for id in self.all_lanes() {
            stops.extend(map.get_l(id).bus_stops.iter().cloned());
        }
        stops
    }

    // Returns [-1.0, 1.0]. 0 is flat, positive is uphill, negative is downhill.
    // TODO Or do we care about the total up/down along the possibly long road?
    pub fn percent_grade(&self, map: &Map) -> f64 {
        let rise = map.get_i(self.dst_i).elevation - map.get_i(self.src_i).elevation;
        let run = self.center_pts.length();
        let grade = rise / run;
        if grade <= -1.0 || grade >= 1.0 {
            // TODO Panic
            //println!("Grade of {} is {}%", self.id, grade * 100.0);
            if grade < 0.0 {
                return -1.0;
            } else {
                return 1.0;
            }
        }
        grade
    }

    pub fn is_light_rail(&self) -> bool {
        !self.children_forwards.is_empty() && self.children_forwards[0].1 == LaneType::LightRail
    }

    pub fn is_private(&self) -> bool {
        self.osm_tags.get("access") == Some(&"private".to_string())
    }
}

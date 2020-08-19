use crate::raw::{OriginalRoad, RestrictionType};
use crate::{osm, BusStopID, IntersectionID, Lane, LaneID, LaneType, Map, PathConstraints, Zone};
use abstutil::{deserialize_usize, serialize_usize, Tags};
use enumset::EnumSet;
use geom::{Distance, PolyLine, Polygon, Speed};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoadID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

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
    pub osm_tags: Tags,
    // self is 'from'
    pub turn_restrictions: Vec<(RestrictionType, RoadID)>,
    // self is 'from'. (via, to). Only BanTurns.
    pub complicated_turn_restrictions: Vec<(RoadID, RoadID)>,
    pub orig_id: OriginalRoad,
    pub speed_limit: Speed,
    pub allow_through_traffic: EnumSet<PathConstraints>,
    pub zorder: isize,

    // Invariant: A road must contain at least one child
    // These are ordered from closest to center lane (left-most when driving on the right) to
    // farthest (sidewalk)
    pub children_forwards: Vec<(LaneID, LaneType)>,
    pub children_backwards: Vec<(LaneID, LaneType)>,

    // The physical center of the road, including sidewalks, after trimming. The order implies road
    // orientation. No edits ever change this.
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

    pub(crate) fn children_mut(&mut self, fwds: bool) -> &mut Vec<(LaneID, LaneType)> {
        if fwds {
            &mut self.children_forwards
        } else {
            &mut self.children_backwards
        }
    }

    pub fn get_lane_types<'a>(&'a self) -> HomogenousTuple2<impl Iterator<Item = LaneType> + 'a> {
        let get_lanetype = |(_, lt): &(_, LaneType)| *lt;
        (
            self.children_forwards.iter().map(get_lanetype.clone()),
            self.children_backwards.iter().map(get_lanetype),
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
            if let Some(idx) = self.children(fwds).iter().position(|pair| pair.0 == lane) {
                return (fwds, idx);
            }
        }
        panic!("{} doesn't contain {}", self.id, lane);
    }

    pub fn parking_to_driving(&self, parking: LaneID, map: &Map) -> Option<LaneID> {
        self.find_closest_lane(parking, |l| l.is_driving(), map)
    }

    pub(crate) fn speed_limit_from_osm(&self) -> Speed {
        if let Some(limit) = self.osm_tags.get(osm::MAXSPEED) {
            if let Ok(kmph) = limit.parse::<f64>() {
                return Speed::km_per_hour(kmph);
            }

            if let Some(mph) = limit
                .strip_suffix(" mph")
                .and_then(|x| x.parse::<f64>().ok())
            {
                return Speed::miles_per_hour(mph);
            }

            // TODO Handle implicits, like PL:zone30
        }

        // These're half reasonable guesses. Better to explicitly tag in OSM.
        if self
            .osm_tags
            .is_any(osm::HIGHWAY, vec!["primary", "secondary", "motorway_link"])
        {
            return Speed::miles_per_hour(40.0);
        }
        if self.osm_tags.is(osm::HIGHWAY, "living_street") {
            // about 12mph
            return Speed::km_per_hour(20.0);
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

    // "Left" depends on the road, so if the lane !is_forwards(), keep that in mind
    pub fn offset_from_left(&self, lane: LaneID) -> usize {
        self.children_backwards
            .iter()
            .rev()
            .chain(self.children_forwards.iter())
            .position(|(l, _)| *l == lane)
            .unwrap()
    }

    // Includes off-side
    // TODO Specialize a variant for PathConstraints.can_use. Only one caller needs something
    // fancier.
    pub fn find_closest_lane<F: Fn(&Lane) -> bool>(
        &self,
        from: LaneID,
        filter: F,
        map: &Map,
    ) -> Option<LaneID> {
        // Lanes from left to right over the whole road
        let mut all = Vec::new();
        for (l, _) in self.children_backwards.iter().rev() {
            all.push(*l);
        }
        for (l, _) in &self.children_forwards {
            all.push(*l);
        }
        let our_idx = all.iter().position(|l| *l == from).unwrap() as isize;

        all.into_iter()
            .enumerate()
            .filter_map(|(idx, l)| {
                if (idx as isize) != our_idx && filter(map.get_l(l)) {
                    Some((idx, l))
                } else {
                    None
                }
            })
            .min_by_key(|(idx, _)| (our_idx - (*idx as isize)).abs())
            .map(|(_, l)| l)
    }

    pub fn all_lanes(&self) -> Vec<LaneID> {
        self.children_forwards
            .iter()
            .chain(self.children_backwards.iter())
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn lanes_on_side<'a>(&'a self, dir: bool) -> impl Iterator<Item = LaneID> + 'a {
        self.children(dir).iter().map(|(id, _)| *id)
    }

    // This is the yellow line where the direction of the road changes.
    pub fn get_current_center(&self, map: &Map) -> PolyLine {
        let lane = map.get_l(if !self.children_forwards.is_empty() {
            self.children_forwards[0].0
        } else {
            self.children_backwards[0].0
        });
        map.must_left_shift(lane.lane_center_pts.clone(), lane.width / 2.0)
    }

    pub fn any_on_other_side(&self, l: LaneID, lt: LaneType) -> Option<LaneID> {
        let search = self.children(!self.is_forwards(l));
        search.iter().find(|(_, t)| lt == *t).map(|(id, _)| *id)
    }

    pub fn get_half_width(&self, map: &Map) -> Distance {
        self.all_lanes()
            .into_iter()
            .map(|l| map.get_l(l).width)
            .sum::<Distance>()
            / 2.0
    }

    pub fn get_thick_polygon(&self, map: &Map) -> Polygon {
        self.center_pts
            .make_polygons(self.get_half_width(map) * 2.0)
    }

    pub fn get_name(&self, lang: Option<&String>) -> String {
        if let Some(lang) = lang {
            if let Some(name) = self.osm_tags.get(&format!("name:{}", lang)) {
                return name.to_string();
            }
        }

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

    pub fn get_rank(&self) -> osm::RoadRank {
        if let Some(x) = self.osm_tags.get(osm::HIGHWAY) {
            if x == "construction" {
                // What exactly is under construction?
                if let Some(x) = self.osm_tags.get("construction") {
                    osm::RoadRank::from_highway(x)
                } else {
                    osm::RoadRank::Local
                }
            } else {
                osm::RoadRank::from_highway(x)
            }
        } else {
            osm::RoadRank::Local
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

    pub fn is_footway(&self) -> bool {
        self.children_forwards.len() == 1
            && self.children_forwards[0].1 == LaneType::Sidewalk
            && self.children_backwards.is_empty()
    }

    pub fn common_endpt(&self, other: &Road) -> IntersectionID {
        if self.src_i == other.src_i || self.src_i == other.dst_i {
            self.src_i
        } else if self.dst_i == other.src_i || self.dst_i == other.dst_i {
            self.dst_i
        } else {
            panic!("{} and {} don't share an endpoint", self.id, other.id);
        }
    }

    pub fn is_private(&self) -> bool {
        self.allow_through_traffic != EnumSet::all()
    }

    pub(crate) fn access_restrictions_from_osm(&self) -> EnumSet<PathConstraints> {
        if self.osm_tags.is("access", "private") {
            EnumSet::new()
        } else if self.osm_tags.is(osm::HIGHWAY, "living_street") {
            let mut allow = PathConstraints::Pedestrian | PathConstraints::Bike;
            if self.osm_tags.is("psv", "yes") || self.osm_tags.is("bus", "yes") {
                allow |= PathConstraints::Bus;
            }
            allow
        } else {
            EnumSet::all()
        }
    }

    pub fn get_zone<'a>(&self, map: &'a Map) -> Option<&'a Zone> {
        if !self.is_private() {
            return None;
        }
        // Insist on it existing
        Some(
            map.zones
                .iter()
                .find(|z| z.members.contains(&self.id))
                .unwrap(),
        )
    }
}

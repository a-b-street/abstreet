//! Everything related to pathfinding through a map for different types of agents.

use enumset::EnumSetType;
use serde::{Deserialize, Serialize};

use geom::Duration;

pub use self::ch::ContractionHierarchyPathfinder;
pub use self::dijkstra::{build_graph_for_pedestrians, build_graph_for_vehicles};
pub use self::pathfinder::Pathfinder;
pub use self::v1::{Path, PathRequest, PathStep};
pub use self::vehicles::vehicle_cost;
pub use self::walking::WalkingNode;
use crate::{osm, Lane, LaneID, LaneType, Map, MovementID};

mod ch;
pub mod dijkstra;
mod node_map;
mod pathfinder;
// TODO tmp
pub mod uber_turns;
mod v1;
mod v2;
mod vehicles;
mod walking;

/// Who's asking for a path?
// TODO This is an awful name.
#[derive(Debug, Serialize, Deserialize, PartialOrd, Ord, EnumSetType)]
pub enum PathConstraints {
    Pedestrian,
    Car,
    Bike,
    Bus,
    Train,
}

impl PathConstraints {
    pub fn all() -> Vec<PathConstraints> {
        vec![
            PathConstraints::Pedestrian,
            PathConstraints::Car,
            PathConstraints::Bike,
            PathConstraints::Bus,
            PathConstraints::Train,
        ]
    }

    /// Not bijective, but this is the best guess of user intent
    pub fn from_lt(lt: LaneType) -> PathConstraints {
        match lt {
            LaneType::Sidewalk | LaneType::Shoulder => PathConstraints::Pedestrian,
            LaneType::Driving => PathConstraints::Car,
            LaneType::Biking => PathConstraints::Bike,
            LaneType::Bus => PathConstraints::Bus,
            LaneType::LightRail => PathConstraints::Train,
            _ => panic!("PathConstraints::from_lt({:?}) doesn't make sense", lt),
        }
    }

    pub fn can_use(self, l: &Lane, map: &Map) -> bool {
        match self {
            PathConstraints::Pedestrian => l.is_walkable(),
            PathConstraints::Car => l.is_driving(),
            PathConstraints::Bike => {
                if l.is_biking() {
                    true
                } else if l.is_driving() || (l.is_bus() && map.config.bikes_can_use_bus_lanes) {
                    let road = map.get_r(l.parent);
                    !road.osm_tags.is("bicycle", "no")
                        && !road
                            .osm_tags
                            .is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
                } else {
                    false
                }
            }
            PathConstraints::Bus => l.is_driving() || l.is_bus(),
            PathConstraints::Train => l.is_light_rail(),
        }
    }

    /// Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub(crate) fn filter_lanes(self, mut choices: Vec<LaneID>, map: &Map) -> Vec<LaneID> {
        choices.retain(|l| self.can_use(map.get_l(*l), map));
        if self == PathConstraints::Bike {
            let just_bike_lanes: Vec<LaneID> = choices
                .iter()
                .copied()
                .filter(|l| map.get_l(*l).is_biking())
                .collect();
            if !just_bike_lanes.is_empty() {
                return just_bike_lanes;
            }
        }
        choices
    }
}

/// Heavily penalize crossing into an access-restricted zone that doesn't allow this mode.
pub fn zone_cost(mvmnt: MovementID, constraints: PathConstraints, map: &Map) -> Duration {
    // Detect when we cross into a new zone that doesn't allow constraints.
    if map
        .get_r(mvmnt.from.id)
        .access_restrictions
        .allow_through_traffic
        .contains(constraints)
        && !map
            .get_r(mvmnt.to.id)
            .access_restrictions
            .allow_through_traffic
            .contains(constraints)
    {
        // This should be high enough to achieve the desired effect of somebody not entering
        // the zone unless absolutely necessary. Someone would violate that and cut through anyway
        // only when the alternative route would take more than 3 hours longer!
        Duration::hours(3)
    } else {
        Duration::ZERO
    }
}

/// Tuneable parameters for all types of routing.
// These will maybe become part of the PathRequest later, but that's an extremely invasive and
// space-expensive change right now.
#[derive(PartialEq, Serialize, Deserialize)]
pub struct RoutingParams {
    // For all vehicles. This is added to the cost of a movement as an additional delay.
    pub unprotected_turn_penalty: Duration,
    // For bike routing. Multiplied by the base cost, since spending more time on the wrong lane
    // type matters.
    pub bike_lane_penalty: f64,
    pub bus_lane_penalty: f64,
    pub driving_lane_penalty: f64,
}

impl RoutingParams {
    pub const fn default() -> RoutingParams {
        RoutingParams {
            // This is a total guess -- it really depends on the traffic patterns of the particular
            // road at the time we're routing.
            unprotected_turn_penalty: Duration::const_seconds(30.0),
            bike_lane_penalty: 1.0,
            bus_lane_penalty: 1.1,
            driving_lane_penalty: 1.5,
        }
    }
}

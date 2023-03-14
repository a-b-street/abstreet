//! Everything related to pathfinding through a map for different types of agents.

use std::collections::BTreeSet;

use enumset::EnumSetType;
use serde::{Deserialize, Serialize};

use geom::Duration;

pub use self::engine::CreateEngine;
pub use self::pathfinder::{Pathfinder, PathfinderCache, PathfinderCaching};
pub use self::v1::{Path, PathRequest, PathStep};
pub use self::v2::{PathStepV2, PathV2};
pub use self::vehicles::vehicle_cost;
pub use self::walking::WalkingNode;
use crate::{osm, Lane, LaneID, LaneType, Map, MovementID, Road, RoadID, TurnType};

mod engine;
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

    /// Can an agent use a lane? There are some subtle exceptions with using bus-only lanes for
    /// turns.
    pub fn can_use(self, lane: &Lane, map: &Map) -> bool {
        let result = match self {
            PathConstraints::Pedestrian => {
                return lane.is_walkable();
            }
            PathConstraints::Car => lane.is_driving(),
            PathConstraints::Bike => {
                if lane.is_biking() {
                    true
                } else if lane.is_driving() || (lane.is_bus() && map.config.bikes_can_use_bus_lanes)
                {
                    let road = map.get_r(lane.id.road);
                    !road.osm_tags.is("bicycle", "no")
                        && !road
                            .osm_tags
                            .is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
                } else {
                    false
                }
            }
            PathConstraints::Bus => {
                return lane.is_driving() || lane.is_bus();
            }
            PathConstraints::Train => {
                return lane.is_light_rail();
            }
        };
        if result {
            return true;
        }
        // Second chance for cars and bikes trying to use a bus-only lane that also happens to be a
        // turn lane.
        //
        // TODO This check could be made stricter in two ways:
        // 1) Verify that the bus-only lanes are the ONLY way to make this movement; if there's a
        //    general purpose lane that can also turn, we shouldn't allow this.
        // 2) Verify that the turn->lane->turn sequence is the only way to reach the destination
        //    road. Since this function operates on a single lane, a vehicle could abuse this to
        //    stay in the bus lane and go straight, even if there was another lane for that. Fixing
        //    this is hard, since it requires so much context about the sequence of movements. In
        //    practice this isn't an issue; a bus lane often leads to another one, but the next bus
        //    lane won't also be an exclusive turn lane.
        if lane.is_bus() {
            if let Some(types) =
                lane.get_lane_level_turn_restrictions(map.get_r(lane.id.road), true)
            {
                if types.contains(&TurnType::Right) || types.contains(&TurnType::Left) {
                    return true;
                }
            }
        }
        false
    }

    /// Can an agent use a road in either direction? There are some subtle exceptions with using
    /// bus-only lanes for turns.
    pub fn can_use_road(self, road: &Road, map: &Map) -> bool {
        road.lanes.iter().any(|lane| self.can_use(lane, map))
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
pub(crate) fn zone_cost(mvmnt: MovementID, constraints: PathConstraints, map: &Map) -> Duration {
    // Detect when we cross into a new zone that doesn't allow constraints.
    if map
        .get_r(mvmnt.from.road)
        .access_restrictions
        .allow_through_traffic
        .contains(constraints)
        && !map
            .get_r(mvmnt.to.road)
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
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RoutingParams {
    // For all vehicles. This is added to the cost of a movement as an additional delay.
    pub unprotected_turn_penalty: Duration,

    // For bike routing. Multiplied by the base cost, since spending more time on the wrong lane
    // type matters.
    pub bike_lane_penalty: f64,
    pub bus_lane_penalty: f64,
    pub driving_lane_penalty: f64,

    // For bike routing.
    // "Steep" is a fixed threshold of 8% incline, uphill only. Multiply by the base cost. (Note
    // that cost already includes a reduction of speed to account for the incline -- this is a
    // further "delay" on top of that!)
    // TODO But even steeper roads matter more!
    pub avoid_steep_incline_penalty: f64,
    // If the road is `high_stress_for_bikes`, multiply by the base cost.
    pub avoid_high_stress: f64,

    /// When crossing an arterial or highway road, multiply the base cost by this penalty. When
    /// greater than 1, this will encourage routes to use local roads more.
    pub main_road_penalty: f64,

    /// Don't allow crossing these roads at all. Only affects vehicle routing, not pedestrian.
    ///
    /// TODO The route may cross one of these roads if it's the start or end!
    pub avoid_roads: BTreeSet<RoadID>,
    /// Related to `avoid_roads`, but used as an optimization in map construction
    pub only_use_roads: BTreeSet<RoadID>,

    /// Don't allow movements between these roads at all. Only affects vehicle routing, not
    /// pedestrian.
    pub avoid_movements_between: BTreeSet<(RoadID, RoadID)>,
}

impl Default for RoutingParams {
    fn default() -> Self {
        Self {
            // This is a total guess -- it really depends on the traffic patterns of the particular
            // road at the time we're routing.
            unprotected_turn_penalty: Duration::const_seconds(30.0),

            bike_lane_penalty: 1.0,
            bus_lane_penalty: 1.1,
            driving_lane_penalty: 1.5,

            avoid_steep_incline_penalty: 1.0,
            avoid_high_stress: 1.0,

            main_road_penalty: 1.0,

            avoid_roads: BTreeSet::new(),
            avoid_movements_between: BTreeSet::new(),
            only_use_roads: BTreeSet::new(),
        }
    }
}

pub fn round(cost: Duration) -> usize {
    // Round up! 0 cost edges are ignored
    (cost.inner_seconds().round() as usize).max(1)
}

pub fn unround(cost: usize) -> Duration {
    Duration::seconds(cost as f64)
}

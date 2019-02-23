use crate::plugins::sim::new_des_model::{
    DistanceInterval, SidewalkPOI, SidewalkSpot, TimeInterval,
};
use abstutil::{deserialize_multimap, serialize_multimap};
use geom::{Distance, Duration, Speed};
use map_model::{Map, Path, PathStep, Traversable};
use multimap::MultiMap;
use serde_derive::{Deserialize, Serialize};
use sim::{DrawPedestrianInput, PedestrianID};
use std::collections::BTreeMap;

// TODO This is comically fast.
const SPEED: Speed = Speed::const_meters_per_second(3.9);

#[derive(Serialize, Deserialize)]
pub struct WalkingSimState {
    // BTreeMap not for deterministic simulation, but to make serialized things easier to compare.
    peds: BTreeMap<PedestrianID, Pedestrian>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    peds_per_traversable: MultiMap<Traversable, PedestrianID>,
}

impl WalkingSimState {
    pub fn new() -> WalkingSimState {
        WalkingSimState {
            peds: BTreeMap::new(),
            peds_per_traversable: MultiMap::new(),
        }
    }

    pub fn spawn_ped(
        &mut self,
        id: PedestrianID,
        start_time: Duration,
        start: SidewalkSpot,
        goal: SidewalkSpot,
        path: Path,
        map: &Map,
    ) {
        let start_lane = start.sidewalk_pos.lane();
        assert_eq!(
            path.current_step().as_traversable(),
            Traversable::Lane(start_lane)
        );
        assert_eq!(
            path.last_step().as_traversable(),
            Traversable::Lane(goal.sidewalk_pos.lane())
        );

        let mut ped = Pedestrian {
            id,
            state: PedState::WaitingToTurn,
            path,
            goal,
        };
        ped.state = match start.connection {
            SidewalkPOI::BikeRack => {
                ped.crossing_state(start.sidewalk_pos.dist_along(), start_time, map)
            }
            _ => panic!("Don't support {:?} yet", start.connection),
        };

        self.peds.insert(id, ped);
        self.peds_per_traversable
            .insert(Traversable::Lane(start.sidewalk_pos.lane()), id);
    }

    pub fn get_all_draw_peds(&self, time: Duration, map: &Map) -> Vec<DrawPedestrianInput> {
        self.peds
            .values()
            .map(|p| p.get_draw_ped(time, map))
            .collect()
    }

    pub fn get_draw_peds(
        &self,
        time: Duration,
        on: Traversable,
        map: &Map,
    ) -> Vec<DrawPedestrianInput> {
        self.peds_per_traversable
            .get_vec(&on)
            .unwrap_or(&Vec::new())
            .iter()
            .map(|id| self.peds[id].get_draw_ped(time, map))
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
struct Pedestrian {
    id: PedestrianID,
    state: PedState,

    path: Path,
    goal: SidewalkSpot,
}

impl Pedestrian {
    fn crossing_state(&self, start_dist: Distance, start_time: Duration, map: &Map) -> PedState {
        let end_dist = if self.path.is_last_step() {
            self.goal.sidewalk_pos.dist_along()
        } else {
            // TODO PathStep should have a end_dist... or end_pos
            match self.path.current_step() {
                PathStep::Lane(l) => map.get_l(l).length(),
                PathStep::ContraflowLane(_) => Distance::ZERO,
                PathStep::Turn(t) => map.get_t(t).geom.length(),
            }
        };
        let dist_int = DistanceInterval::new_walking(start_dist, end_dist);
        let time_int = TimeInterval::new(start_time, start_time + dist_int.length() / SPEED);
        PedState::Crossing(dist_int, time_int)
    }

    fn get_draw_ped(&self, time: Duration, map: &Map) -> DrawPedestrianInput {
        let (on, dist) = match self.state {
            PedState::Crossing(ref dist_int, ref time_int) => (
                self.path.current_step().as_traversable(),
                dist_int.lerp(time_int.percent(time)),
            ),
            PedState::WaitingToTurn => match self.path.current_step() {
                PathStep::Lane(l) => (Traversable::Lane(l), map.get_l(l).length()),
                PathStep::ContraflowLane(l) => (Traversable::Lane(l), Distance::ZERO),
                PathStep::Turn(t) => (Traversable::Turn(t), map.get_t(t).geom.length()),
            },
        };

        DrawPedestrianInput {
            id: self.id,
            pos: on.dist_along(dist, map).0,
            waiting_for_turn: None,
            preparing_bike: false,
            on,
        }
    }
}

#[derive(Serialize, Deserialize)]
enum PedState {
    // crossing front path, bike parking, waiting at bus stop,
    Crossing(DistanceInterval, TimeInterval),
    WaitingToTurn,
}

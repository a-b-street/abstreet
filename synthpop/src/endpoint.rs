use geom::Pt2D;
use map_model::{BuildingID, IntersectionID, Map, PathConstraints, PathRequest, Position};
use serde::{Deserialize, Serialize};

use crate::TripMode;

/// Specifies where a trip begins or ends.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum TripEndpoint {
    Building(BuildingID),
    Border(IntersectionID),
    /// Used for interactive spawning, tests, etc. For now, only valid as a trip's start.
    SuddenlyAppear(Position),
}

impl TripEndpoint {
    /// Returns a point representing where this endpoint is.
    pub fn pt(self, map: &Map) -> Pt2D {
        match self {
            TripEndpoint::Building(b) => map.get_b(b).polygon.center(),
            TripEndpoint::Border(i) => map.get_i(i).polygon.center(),
            TripEndpoint::SuddenlyAppear(pos) => pos.pt(map),
        }
    }

    /// Figure out a single PathRequest that goes between two TripEndpoints. Assume a single mode
    /// the entire time -- no walking to a car before driving, for instance. The result probably
    /// won't be exactly what would happen on a real trip between the endpoints because of this
    /// assumption.
    pub fn path_req(
        from: TripEndpoint,
        to: TripEndpoint,
        mode: TripMode,
        map: &Map,
    ) -> Option<PathRequest> {
        let start = from.pos(mode, true, map)?;
        let end = to.pos(mode, false, map)?;
        Some(match mode {
            TripMode::Walk | TripMode::Transit => PathRequest::walking(start, end),
            TripMode::Bike => PathRequest::vehicle(start, end, PathConstraints::Bike),
            // Only cars leaving from a building might turn out from the driveway in a special way
            TripMode::Drive => {
                if matches!(from, TripEndpoint::Building(_)) {
                    PathRequest::leave_from_driveway(start, end, PathConstraints::Car, map)
                } else {
                    PathRequest::vehicle(start, end, PathConstraints::Car)
                }
            }
        })
    }

    fn pos(self, mode: TripMode, from: bool, map: &Map) -> Option<Position> {
        match mode {
            TripMode::Walk | TripMode::Transit => self.sidewalk_pos(map, from),
            TripMode::Drive | TripMode::Bike => {
                let constraints = mode.to_constraints();
                if from {
                    match self {
                        // Fall through
                        TripEndpoint::Building(_) => {}
                        TripEndpoint::Border(i) => {
                            return map.get_i(i).some_outgoing_road(map).and_then(|dr| {
                                dr.lanes(constraints, map)
                                    .get(0)
                                    .map(|l| Position::start(*l))
                            });
                        }
                        TripEndpoint::SuddenlyAppear(pos) => {
                            return Some(pos);
                        }
                    }
                }

                match self {
                    TripEndpoint::Building(b) => match constraints {
                        PathConstraints::Car => {
                            let driving_lane = map.find_driving_lane_near_building(b);
                            let sidewalk_pos = map.get_b(b).sidewalk_pos;
                            if driving_lane.road == sidewalk_pos.lane().road {
                                Some(sidewalk_pos.equiv_pos(driving_lane, map))
                            } else {
                                Some(Position::start(driving_lane))
                            }
                        }
                        PathConstraints::Bike => Some(map.get_b(b).biking_connection(map)?.0),
                        PathConstraints::Bus
                        | PathConstraints::Train
                        | PathConstraints::Pedestrian => {
                            unreachable!()
                        }
                    },
                    TripEndpoint::Border(i) => {
                        map.get_i(i).some_incoming_road(map).and_then(|dr| {
                            let lanes = dr.lanes(constraints, map);
                            if lanes.is_empty() {
                                None
                            } else {
                                // TODO ideally could use any
                                Some(Position::end(lanes[0], map))
                            }
                        })
                    }
                    TripEndpoint::SuddenlyAppear(_) => unreachable!(),
                }
            }
        }
    }

    fn sidewalk_pos(self, map: &Map, from: bool) -> Option<Position> {
        match self {
            TripEndpoint::Building(b) => Some(map.get_b(b).sidewalk_pos),
            TripEndpoint::Border(i) => {
                if from {
                    TripEndpoint::start_walking_at_border(i, map)
                } else {
                    TripEndpoint::end_walking_at_border(i, map)
                }
            }
            TripEndpoint::SuddenlyAppear(pos) => Some(pos),
        }
    }

    // Recall sidewalks are bidirectional.
    pub fn start_walking_at_border(i: IntersectionID, map: &Map) -> Option<Position> {
        let lanes = map
            .get_i(i)
            .get_outgoing_lanes(map, PathConstraints::Pedestrian);
        if !lanes.is_empty() {
            return Some(Position::start(lanes[0]));
        }
        map.get_i(i)
            .get_incoming_lanes(map, PathConstraints::Pedestrian)
            .get(0)
            .map(|l| Position::end(*l, map))
    }

    pub fn end_walking_at_border(i: IntersectionID, map: &Map) -> Option<Position> {
        if let Some(l) = map
            .get_i(i)
            .get_incoming_lanes(map, PathConstraints::Pedestrian)
            .get(0)
        {
            return Some(Position::end(*l, map));
        }

        let lanes = map
            .get_i(i)
            .get_outgoing_lanes(map, PathConstraints::Pedestrian);
        if lanes.is_empty() {
            return None;
        }
        Some(Position::start(lanes[0]))
    }
}

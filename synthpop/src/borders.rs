use geom::{LonLat, Pt2D};
use map_model::osm::RoadRank;
use map_model::{Intersection, IntersectionID, Map, PathConstraints};

use crate::TripMode;

/// Lists all border intersections of the map, broken down by mode and whether they support
/// incoming or outgoing traffic.
#[derive(Clone)]
pub struct MapBorders {
    pub incoming_walking: Vec<MapBorder>,
    pub incoming_driving: Vec<MapBorder>,
    pub incoming_biking: Vec<MapBorder>,
    pub outgoing_walking: Vec<MapBorder>,
    pub outgoing_driving: Vec<MapBorder>,
    pub outgoing_biking: Vec<MapBorder>,
}

#[derive(Clone)]
pub struct MapBorder {
    pub i: IntersectionID,
    pub pos: Pt2D,
    pub gps_pos: LonLat,
    /// Based on the classification of the connecting road, a weight for how likely this border is
    /// to be used for traffic.
    pub weight: usize,
}

impl MapBorders {
    pub fn new(map: &Map) -> MapBorders {
        let incoming_walking = map
            .all_incoming_borders()
            .into_iter()
            .filter(|i| {
                !i.get_outgoing_lanes(map, PathConstraints::Pedestrian)
                    .is_empty()
            })
            .map(|i| MapBorder::new(map, i))
            .collect::<Vec<_>>();
        let incoming_driving = map
            .all_incoming_borders()
            .into_iter()
            .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Car).is_empty())
            .map(|i| MapBorder::new(map, i))
            .collect::<Vec<_>>();
        let incoming_biking = map
            .all_incoming_borders()
            .into_iter()
            .filter(|i| !i.get_outgoing_lanes(map, PathConstraints::Bike).is_empty())
            .map(|i| MapBorder::new(map, i))
            .collect::<Vec<_>>();
        let outgoing_walking = map
            .all_outgoing_borders()
            .into_iter()
            .filter(|i| {
                !i.get_incoming_lanes(map, PathConstraints::Pedestrian)
                    .is_empty()
            })
            .map(|i| MapBorder::new(map, i))
            .collect::<Vec<_>>();
        let outgoing_driving = map
            .all_outgoing_borders()
            .into_iter()
            .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Car).is_empty())
            .map(|i| MapBorder::new(map, i))
            .collect::<Vec<_>>();
        let outgoing_biking = map
            .all_outgoing_borders()
            .into_iter()
            .filter(|i| !i.get_incoming_lanes(map, PathConstraints::Bike).is_empty())
            .map(|i| MapBorder::new(map, i))
            .collect::<Vec<_>>();
        MapBorders {
            incoming_walking,
            incoming_driving,
            incoming_biking,
            outgoing_walking,
            outgoing_driving,
            outgoing_biking,
        }
    }

    /// Returns the (incoming, outgoing) borders for the specififed mode.
    pub fn for_mode(&self, mode: TripMode) -> (&Vec<MapBorder>, &Vec<MapBorder>) {
        match mode {
            TripMode::Walk | TripMode::Transit => (&self.incoming_walking, &self.outgoing_walking),
            TripMode::Drive => (&self.incoming_driving, &self.outgoing_driving),
            TripMode::Bike => (&self.incoming_biking, &self.outgoing_biking),
        }
    }
}

impl MapBorder {
    fn new(map: &Map, i: &Intersection) -> Self {
        // TODO Mostly untuned, and agnostic to TripMode
        let road = map.get_r(*i.roads.iter().next().unwrap());
        let mut weight = match road.get_rank() {
            RoadRank::Local => 3,
            RoadRank::Arterial => 5,
            RoadRank::Highway => 8,
        };
        // TODO We should consider more values for RoadRank
        if road.is_service() {
            weight = 1;
        }

        let pos = i.polygon.center();
        Self {
            i: i.id,
            pos,
            gps_pos: pos.to_gps(map.get_gps_bounds()),
            weight,
        }
    }
}

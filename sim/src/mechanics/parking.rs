use crate::{CarID, CarStatus, DrawCarInput, ParkedCar, ParkingSpot, Vehicle};
use abstutil::{
    deserialize_btreemap, deserialize_multimap, serialize_btreemap, serialize_multimap, MultiMap,
};
use geom::Distance;
use map_model;
use map_model::{BuildingID, Lane, LaneID, LaneType, Map, Position, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::iter;

#[derive(Serialize, Deserialize, PartialEq)]
pub struct ParkingSimState {
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    cars: BTreeMap<CarID, ParkedCar>,
    lanes: BTreeMap<LaneID, ParkingLane>,
    reserved_spots: BTreeSet<ParkingSpot>,

    driving_to_parking_lane: BTreeMap<LaneID, LaneID>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    cars_per_building: MultiMap<BuildingID, CarID>,
}

impl ParkingSimState {
    pub fn new(map: &Map) -> ParkingSimState {
        let mut sim = ParkingSimState {
            cars: BTreeMap::new(),
            lanes: BTreeMap::new(),
            reserved_spots: BTreeSet::new(),
            driving_to_parking_lane: BTreeMap::new(),
            cars_per_building: MultiMap::new(),
        };
        for l in map.all_lanes() {
            if let Some(lane) = ParkingLane::new(l, map) {
                assert!(!sim.driving_to_parking_lane.contains_key(&lane.driving_lane));
                sim.driving_to_parking_lane.insert(lane.driving_lane, l.id);
                sim.lanes.insert(lane.id, lane);
            }
        }
        sim
    }

    pub fn get_free_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        let lane = &self.lanes[&l];
        let mut spots: Vec<ParkingSpot> = Vec::new();
        for (idx, maybe_occupant) in lane.occupants.iter().enumerate() {
            if maybe_occupant.is_none() {
                spots.push(ParkingSpot::new(lane.id, idx));
            }
        }
        spots
    }

    pub fn remove_parked_car(&mut self, p: ParkedCar) {
        self.cars.remove(&p.vehicle.id);
        self.lanes
            .get_mut(&p.spot.lane)
            .unwrap()
            .remove_parked_car(p.vehicle.id);
    }

    pub fn add_parked_car(&mut self, p: ParkedCar) {
        let spot = p.spot;
        assert!(self.reserved_spots.remove(&p.spot));
        assert_eq!(self.lanes[&spot.lane].occupants[spot.idx], None);
        self.lanes.get_mut(&spot.lane).unwrap().occupants[spot.idx] = Some(p.vehicle.id);
        if let Some(b) = p.vehicle.owner {
            self.cars_per_building.insert(b, p.vehicle.id);
        }
        self.cars.insert(p.vehicle.id, p);
    }

    pub fn reserve_spot(&mut self, spot: ParkingSpot) {
        self.reserved_spots.insert(spot);
    }

    pub fn get_draw_cars(&self, id: LaneID, map: &Map) -> Vec<DrawCarInput> {
        if let Some(ref lane) = self.lanes.get(&id) {
            lane.occupants
                .iter()
                .filter_map(|maybe_occupant| {
                    if let Some(car) = maybe_occupant {
                        Some(self.get_draw_car(*car, map).unwrap())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        let p = self.cars.get(&id)?;
        let lane = p.spot.lane;

        let front_dist = self.lanes[&lane].dist_along_for_car(p.spot.idx, &p.vehicle);
        Some(DrawCarInput {
            id: p.vehicle.id,
            waiting_for_turn: None,
            status: CarStatus::Parked,
            on: Traversable::Lane(lane),
            label: None,

            body: map
                .get_l(lane)
                .lane_center_pts
                .exact_slice(front_dist - p.vehicle.length, front_dist),
        })
    }

    pub fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        self.cars
            .keys()
            .map(|id| self.get_draw_car(*id, map).unwrap())
            .collect()
    }

    pub fn is_free(&self, spot: ParkingSpot) -> bool {
        self.lanes[&spot.lane].occupants[spot.idx].is_none() && !self.reserved_spots.contains(&spot)
    }

    pub fn get_car_at_spot(&self, spot: ParkingSpot) -> Option<ParkedCar> {
        let car = self.lanes[&spot.lane].occupants[spot.idx]?;
        Some(self.cars[&car].clone())
    }

    // And the driving position
    pub fn get_first_free_spot(
        &self,
        driving_pos: Position,
        vehicle: &Vehicle,
        map: &Map,
    ) -> Option<(ParkingSpot, Position)> {
        let l = *self.driving_to_parking_lane.get(&driving_pos.lane())?;
        let parking_dist = driving_pos.equiv_pos(l, map).dist_along();
        let lane = &self.lanes[&l];
        let idx = lane.occupants.iter().enumerate().position(|(idx, x)| {
            x.is_none()
                && !self.reserved_spots.contains(&ParkingSpot::new(l, idx))
                && parking_dist <= lane.dist_along_for_car(idx, vehicle)
        })?;
        let spot = ParkingSpot::new(l, idx);
        Some((spot, self.spot_to_driving_pos(spot, vehicle, map)))
    }

    pub fn spot_to_driving_pos(&self, spot: ParkingSpot, vehicle: &Vehicle, map: &Map) -> Position {
        Position::new(
            spot.lane,
            self.lanes[&spot.lane].dist_along_for_car(spot.idx, vehicle),
        )
        .equiv_pos(self.lanes[&spot.lane].driving_lane, map)
    }

    pub fn spot_to_sidewalk_pos(&self, spot: ParkingSpot, sidewalk: LaneID, map: &Map) -> Position {
        // Always centered in the entire parking spot
        Position::new(
            spot.lane,
            self.lanes[&spot.lane].spot_dist_along[spot.idx]
                - (map_model::PARKING_SPOT_LENGTH / 2.0),
        )
        .equiv_pos(sidewalk, map)
    }

    pub fn tooltip_lines(&self, id: CarID) -> Option<Vec<String>> {
        let c = self.cars.get(&id)?;
        Some(vec![format!(
            "{} is parked, owned by {:?}",
            c.vehicle.id, c.vehicle.owner
        )])
    }

    pub fn get_parked_cars_by_owner(&self, id: BuildingID) -> Vec<&ParkedCar> {
        self.cars_per_building
            .get(id)
            .iter()
            .filter_map(|id| self.cars.get(&id))
            .collect()
    }

    pub fn get_owner_of_car(&self, id: CarID) -> Option<BuildingID> {
        self.cars.get(&id).and_then(|p| p.vehicle.owner)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct ParkingLane {
    id: LaneID,
    driving_lane: LaneID,
    // The front of the parking spot (farthest along the lane)
    spot_dist_along: Vec<Distance>,
    occupants: Vec<Option<CarID>>,
}

impl ParkingLane {
    fn new(l: &Lane, map: &Map) -> Option<ParkingLane> {
        if l.lane_type != LaneType::Parking {
            return None;
        }

        let driving_lane = if let Some(l) = map.get_parent(l.id).parking_to_driving(l.id) {
            l
        } else {
            // TODO Should be a warning
            println!("Parking lane {} has no driving lane!", l.id);
            return None;
        };

        Some(ParkingLane {
            id: l.id,
            driving_lane,
            occupants: iter::repeat(None).take(l.number_parking_spots()).collect(),
            spot_dist_along: (0..l.number_parking_spots())
                .map(|idx| map_model::PARKING_SPOT_LENGTH * (2.0 + idx as f64))
                .collect(),
        })
    }

    fn remove_parked_car(&mut self, car: CarID) {
        let idx = self.occupants.iter().position(|x| *x == Some(car)).unwrap();
        self.occupants[idx] = None;
    }

    fn dist_along_for_car(&self, spot_idx: usize, vehicle: &Vehicle) -> Distance {
        // Find the offset to center this particular car in the parking spot
        self.spot_dist_along[spot_idx] - (map_model::PARKING_SPOT_LENGTH - vehicle.length) / 2.0
    }
}

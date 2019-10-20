use crate::{AgentMetadata, CarID, CarStatus, DrawCarInput, ParkedCar, ParkingSpot, Vehicle};
use abstutil::{
    deserialize_btreemap, deserialize_multimap, serialize_btreemap, serialize_multimap, MultiMap,
};
use geom::{Distance, Duration, Pt2D};
use map_model;
use map_model::{BuildingID, Lane, LaneID, LaneType, Map, Position, Traversable};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize, PartialEq)]
pub struct ParkingSimState {
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    parked_cars: BTreeMap<CarID, ParkedCar>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    occupants: BTreeMap<ParkingSpot, CarID>,
    reserved_spots: BTreeSet<ParkingSpot>,
    dynamically_reserved_cars: BTreeSet<CarID>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    owned_cars_per_building: MultiMap<BuildingID, CarID>,

    // On-street specific
    onstreet_lanes: BTreeMap<LaneID, ParkingLane>,
    // TODO Really this could be 0, 1, or 2 lanes. Full MultiMap is overkill.
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    driving_to_parking_lanes: MultiMap<LaneID, LaneID>,

    // Off-street specific
    num_spots_per_offstreet: BTreeMap<BuildingID, usize>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    driving_to_offstreet: MultiMap<LaneID, BuildingID>,
}

impl ParkingSimState {
    // Counterintuitive: any spots located in blackholes are just not represented here. If somebody
    // tries to drive from a blackholed spot, they couldn't reach most places.
    pub fn new(map: &Map) -> ParkingSimState {
        let mut sim = ParkingSimState {
            parked_cars: BTreeMap::new(),
            occupants: BTreeMap::new(),
            dynamically_reserved_cars: BTreeSet::new(),
            reserved_spots: BTreeSet::new(),
            owned_cars_per_building: MultiMap::new(),

            onstreet_lanes: BTreeMap::new(),
            driving_to_parking_lanes: MultiMap::new(),
            num_spots_per_offstreet: BTreeMap::new(),
            driving_to_offstreet: MultiMap::new(),
        };
        for l in map.all_lanes() {
            if let Some(lane) = ParkingLane::new(l, map) {
                sim.driving_to_parking_lanes.insert(lane.driving_lane, l.id);
                sim.onstreet_lanes.insert(lane.parking_lane, lane);
            }
        }
        for b in map.all_buildings() {
            if let Some(ref p) = b.parking {
                if map.get_l(p.driving_pos.lane()).parking_blackhole.is_some() {
                    continue;
                }
                sim.num_spots_per_offstreet.insert(b.id, p.num_stalls);
                sim.driving_to_offstreet.insert(p.driving_pos.lane(), b.id);
            }
        }
        sim
    }

    pub fn get_free_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        let mut spots: Vec<ParkingSpot> = Vec::new();
        if let Some(lane) = self.onstreet_lanes.get(&l) {
            for spot in lane.spots() {
                if self.is_free(spot) {
                    spots.push(spot);
                }
            }
        }
        for b in self.driving_to_offstreet.get(l) {
            spots.extend(self.get_free_offstreet_spots(*b));
        }
        spots
    }

    pub fn get_free_offstreet_spots(&self, b: BuildingID) -> Vec<ParkingSpot> {
        let mut spots: Vec<ParkingSpot> = Vec::new();
        for idx in 0..self.num_spots_per_offstreet.get(&b).cloned().unwrap_or(0) {
            let spot = ParkingSpot::offstreet(b, idx);
            if self.is_free(spot) {
                spots.push(spot);
            }
        }
        spots
    }

    pub fn reserve_spot(&mut self, spot: ParkingSpot) {
        assert!(self.is_free(spot));
        self.reserved_spots.insert(spot);
    }

    pub fn remove_parked_car(&mut self, p: ParkedCar) {
        self.parked_cars
            .remove(&p.vehicle.id)
            .expect("remove_parked_car missing from parked_cars");
        self.occupants
            .remove(&p.spot)
            .expect("remove_parked_car missing from occupants");
        if let Some(b) = p.vehicle.owner {
            self.owned_cars_per_building.remove(b, p.vehicle.id);
        }
        self.dynamically_reserved_cars.remove(&p.vehicle.id);
    }

    pub fn add_parked_car(&mut self, p: ParkedCar) {
        assert!(self.reserved_spots.remove(&p.spot));
        self.occupants.insert(p.spot, p.vehicle.id);
        if let Some(b) = p.vehicle.owner {
            self.owned_cars_per_building.insert(b, p.vehicle.id);
        }
        self.parked_cars.insert(p.vehicle.id, p);
    }

    pub fn dynamically_reserve_car(&mut self, b: BuildingID) -> Option<ParkedCar> {
        for c in self.owned_cars_per_building.get(b) {
            if self.dynamically_reserved_cars.contains(c) {
                continue;
            }
            self.dynamically_reserved_cars.insert(*c);
            return Some(self.parked_cars[c].clone());
        }
        None
    }

    pub fn dynamically_return_car(&mut self, p: ParkedCar) {
        self.dynamically_reserved_cars.remove(&p.vehicle.id);
    }

    pub fn get_draw_cars(&self, id: LaneID, map: &Map) -> Vec<DrawCarInput> {
        let mut cars = Vec::new();
        if let Some(ref lane) = self.onstreet_lanes.get(&id) {
            for spot in lane.spots() {
                if let Some(car) = self.occupants.get(&spot) {
                    cars.push(self.get_draw_car(*car, map).unwrap());
                }
            }
        }
        cars
    }

    pub fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        let p = self.parked_cars.get(&id)?;
        match p.spot {
            ParkingSpot::Onstreet(lane, idx) => {
                let front_dist = self.onstreet_lanes[&lane].dist_along_for_car(idx, &p.vehicle);
                Some(DrawCarInput {
                    id: p.vehicle.id,
                    waiting_for_turn: None,
                    status: CarStatus::Parked,
                    on: Traversable::Lane(lane),
                    label: None,
                    metadata: AgentMetadata {
                        time_spent_blocked: Duration::ZERO,
                        percent_dist_crossed: 0.0,
                        trip_time_so_far: Duration::ZERO,
                    },

                    body: map
                        .get_l(lane)
                        .lane_center_pts
                        .exact_slice(front_dist - p.vehicle.length, front_dist),
                })
            }
            ParkingSpot::Offstreet(_, _) => None,
        }
    }

    // There's no DrawCarInput for cars parked offstreet, so we need this.
    pub fn canonical_pt(&self, id: CarID, map: &Map) -> Option<Pt2D> {
        let p = self.parked_cars.get(&id)?;
        match p.spot {
            ParkingSpot::Onstreet(_, _) => self.get_draw_car(id, map).map(|c| c.body.last_pt()),
            ParkingSpot::Offstreet(b, _) => Some(map.get_b(b).label_center),
        }
    }

    pub fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        self.parked_cars
            .keys()
            .filter_map(|id| self.get_draw_car(*id, map))
            .collect()
    }

    pub fn is_free(&self, spot: ParkingSpot) -> bool {
        !self.occupants.contains_key(&spot) && !self.reserved_spots.contains(&spot)
    }

    pub fn get_car_at_spot(&self, spot: ParkingSpot) -> Option<&ParkedCar> {
        let car = self.occupants.get(&spot)?;
        Some(&self.parked_cars[&car])
    }

    // And the driving position
    // TODO This one is trickier!
    pub fn get_first_free_spot(
        &self,
        driving_pos: Position,
        vehicle: &Vehicle,
        map: &Map,
    ) -> Option<(ParkingSpot, Position)> {
        let mut maybe_spot = None;
        // TODO Ideally don't fill in one side first before considering the other.
        for l in self.driving_to_parking_lanes.get(driving_pos.lane()) {
            let parking_dist = driving_pos.equiv_pos(*l, vehicle.length, map).dist_along();
            let lane = &self.onstreet_lanes[l];
            // Bit hacky to enumerate here to conveniently get idx.
            for (idx, spot) in lane.spots().into_iter().enumerate() {
                if self.is_free(spot) && parking_dist <= lane.dist_along_for_car(idx, vehicle) {
                    maybe_spot = Some(spot);
                    break;
                }
            }
        }

        for b in self.driving_to_offstreet.get(driving_pos.lane()) {
            let bldg_dist = map
                .get_b(*b)
                .parking
                .as_ref()
                .unwrap()
                .driving_pos
                .dist_along();
            if driving_pos.dist_along() > bldg_dist {
                continue;
            }
            // Is this potential spot closer than the current best spot?
            if maybe_spot
                .map(|spot| bldg_dist > self.spot_to_driving_pos(spot, vehicle, map).dist_along())
                .unwrap_or(false)
            {
                continue;
            }

            for idx in 0..self.num_spots_per_offstreet[&b] {
                let spot = ParkingSpot::offstreet(*b, idx);
                if self.is_free(spot) {
                    maybe_spot = Some(spot);
                    break;
                }
            }
        }

        let spot = maybe_spot?;
        Some((spot, self.spot_to_driving_pos(spot, vehicle, map)))
    }

    pub fn spot_to_driving_pos(&self, spot: ParkingSpot, vehicle: &Vehicle, map: &Map) -> Position {
        match spot {
            ParkingSpot::Onstreet(l, idx) => {
                let lane = &self.onstreet_lanes[&l];
                Position::new(l, lane.dist_along_for_car(idx, vehicle)).equiv_pos(
                    lane.driving_lane,
                    vehicle.length,
                    map,
                )
            }
            ParkingSpot::Offstreet(b, _) => map.get_b(b).parking.as_ref().unwrap().driving_pos,
        }
    }

    pub fn spot_to_sidewalk_pos(&self, spot: ParkingSpot, map: &Map) -> Position {
        match spot {
            ParkingSpot::Onstreet(l, idx) => {
                // TODO Consider precomputing this.
                let sidewalk = map.find_closest_lane(l, vec![LaneType::Sidewalk]).unwrap();
                // Always centered in the entire parking spot
                Position::new(
                    l,
                    self.onstreet_lanes[&l].spot_dist_along[idx]
                        - (map_model::PARKING_SPOT_LENGTH / 2.0),
                )
                .equiv_pos(sidewalk, Distance::ZERO, map)
            }
            ParkingSpot::Offstreet(b, _) => map.get_b(b).front_path.sidewalk,
        }
    }

    pub fn tooltip_lines(&self, id: CarID) -> Option<Vec<String>> {
        let c = self.parked_cars.get(&id)?;
        Some(vec![format!(
            "{} is parked, owned by {:?}",
            c.vehicle.id, c.vehicle.owner
        )])
    }

    pub fn get_parked_cars_by_owner(&self, b: BuildingID) -> Vec<&ParkedCar> {
        self.owned_cars_per_building
            .get(b)
            .iter()
            .filter_map(|car| self.parked_cars.get(&car))
            .collect()
    }

    pub fn get_offstreet_parked_cars(&self, b: BuildingID) -> Vec<&ParkedCar> {
        let mut results = Vec::new();
        for idx in 0..self.num_spots_per_offstreet.get(&b).cloned().unwrap_or(0) {
            if let Some(car) = self.occupants.get(&ParkingSpot::offstreet(b, idx)) {
                results.push(&self.parked_cars[&car]);
            }
        }
        results
    }

    pub fn get_owner_of_car(&self, id: CarID) -> Option<BuildingID> {
        self.parked_cars.get(&id).and_then(|p| p.vehicle.owner)
    }

    // (Filled, available)
    pub fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        let mut filled = Vec::new();
        let mut available = Vec::new();

        for lane in self.onstreet_lanes.values() {
            for spot in lane.spots() {
                if self.is_free(spot) {
                    available.push(spot);
                } else {
                    filled.push(spot);
                }
            }
        }
        for (b, idx) in &self.num_spots_per_offstreet {
            let spot = ParkingSpot::Offstreet(*b, *idx);
            if self.is_free(spot) {
                available.push(spot);
            } else {
                filled.push(spot);
            }
        }

        (filled, available)
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
struct ParkingLane {
    parking_lane: LaneID,
    driving_lane: LaneID,
    // The front of the parking spot (farthest along the lane)
    spot_dist_along: Vec<Distance>,
}

impl ParkingLane {
    fn new(l: &Lane, map: &Map) -> Option<ParkingLane> {
        if l.lane_type != LaneType::Parking {
            return None;
        }

        let driving_lane = if let Some(l) = map.get_parent(l.id).parking_to_driving(l.id) {
            l
        } else {
            panic!("Parking lane {} has no driving lane!", l.id);
        };
        if map.get_l(driving_lane).parking_blackhole.is_some() {
            return None;
        }

        Some(ParkingLane {
            parking_lane: l.id,
            driving_lane,
            spot_dist_along: (0..l.number_parking_spots())
                .map(|idx| map_model::PARKING_SPOT_LENGTH * (2.0 + idx as f64))
                .collect(),
        })
    }

    fn dist_along_for_car(&self, spot_idx: usize, vehicle: &Vehicle) -> Distance {
        // Find the offset to center this particular car in the parking spot
        self.spot_dist_along[spot_idx] - (map_model::PARKING_SPOT_LENGTH - vehicle.length) / 2.0
    }

    fn spots(&self) -> Vec<ParkingSpot> {
        let mut spots = Vec::new();
        for idx in 0..self.spot_dist_along.len() {
            spots.push(ParkingSpot::onstreet(self.parking_lane, idx));
        }
        spots
    }
}

use crate::{CarID, CarStatus, DrawCarInput, Event, ParkedCar, ParkingSpot, PersonID, Vehicle};
use abstutil::{
    deserialize_btreemap, deserialize_multimap, serialize_btreemap, serialize_multimap, MultiMap,
    Timer,
};
use enum_dispatch::enum_dispatch;
use geom::{Distance, PolyLine, Pt2D};
use map_model::{
    BuildingID, Lane, LaneID, LaneType, Map, OffstreetParking, ParkingLotID, PathConstraints,
    PathStep, Position, Traversable, TurnID,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};

#[enum_dispatch(ParkingSimState)]
pub trait ParkingSim {
    // Returns any cars that got very abruptly evicted from existence
    fn handle_live_edits(&mut self, map: &Map, timer: &mut Timer) -> Vec<ParkedCar>;
    fn get_free_onstreet_spots(&self, l: LaneID) -> Vec<ParkingSpot>;
    fn get_free_offstreet_spots(&self, b: BuildingID) -> Vec<ParkingSpot>;
    fn get_free_lot_spots(&self, pl: ParkingLotID) -> Vec<ParkingSpot>;
    fn reserve_spot(&mut self, spot: ParkingSpot);
    fn remove_parked_car(&mut self, p: ParkedCar);
    fn add_parked_car(&mut self, p: ParkedCar);
    fn get_draw_cars(&self, id: LaneID, map: &Map) -> Vec<DrawCarInput>;
    fn get_draw_cars_in_lots(&self, id: LaneID, map: &Map) -> Vec<DrawCarInput>;
    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput>;
    // There's no DrawCarInput for cars parked offstreet, so we need this.
    fn canonical_pt(&self, id: CarID, map: &Map) -> Option<Pt2D>;
    fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput>;
    fn is_free(&self, spot: ParkingSpot) -> bool;
    fn get_car_at_spot(&self, spot: ParkingSpot) -> Option<&ParkedCar>;
    // The vehicle's front is currently at the given driving_pos. Returns all valid spots and their
    // driving position.
    fn get_all_free_spots(
        &self,
        driving_pos: Position,
        vehicle: &Vehicle,
        // Either the building where a seeded car starts or the target of a trip. For filtering
        // private spots.
        target: BuildingID,
        map: &Map,
    ) -> Vec<(ParkingSpot, Position)>;
    fn spot_to_driving_pos(&self, spot: ParkingSpot, vehicle: &Vehicle, map: &Map) -> Position;
    fn spot_to_sidewalk_pos(&self, spot: ParkingSpot, map: &Map) -> Position;
    fn get_owner_of_car(&self, id: CarID) -> Option<PersonID>;
    fn lookup_parked_car(&self, id: CarID) -> Option<&ParkedCar>;
    // (Filled, available)
    fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>);
    // Unrealistically assumes the driver has knowledge of currently free parking spots, even if
    // they're far away. Since they don't reserve the spot in advance, somebody else can still beat
    // them there, producing some nice, realistic churn if there's too much contention.
    // The first PathStep is the turn after start, NOT PathStep::Lane(start).
    fn path_to_free_parking_spot(
        &self,
        start: LaneID,
        vehicle: &Vehicle,
        target: BuildingID,
        map: &Map,
    ) -> Option<(Vec<PathStep>, ParkingSpot, Position)>;
    fn collect_events(&mut self) -> Vec<Event>;
    fn all_parked_car_positions(&self, map: &Map) -> Vec<(Position, PersonID)>;
    fn bldg_to_parked_cars(&self, b: BuildingID) -> Vec<CarID>;
}

#[enum_dispatch]
#[derive(Serialize, Deserialize, Clone)]
pub enum ParkingSimState {
    Normal(NormalParkingSimState),
    Infinite(InfiniteParkingSimState),
}

impl ParkingSimState {
    // Counterintuitive: any spots located in blackholes are just not represented here. If somebody
    // tries to drive from a blackholed spot, they couldn't reach most places.
    pub fn new(map: &Map, infinite: bool, timer: &mut Timer) -> ParkingSimState {
        if infinite {
            ParkingSimState::Infinite(InfiniteParkingSimState::new(map))
        } else {
            ParkingSimState::Normal(NormalParkingSimState::new(map, timer))
        }
    }

    pub fn is_infinite(&self) -> bool {
        match self {
            ParkingSimState::Normal(_) => false,
            ParkingSimState::Infinite(_) => true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NormalParkingSimState {
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

    // On-street
    onstreet_lanes: BTreeMap<LaneID, ParkingLane>,
    // TODO Really this could be 0, 1, or 2 lanes. Full MultiMap is overkill.
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    driving_to_parking_lanes: MultiMap<LaneID, LaneID>,

    // Off-street
    num_spots_per_offstreet: BTreeMap<BuildingID, usize>,
    // Cache dist_along
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    driving_to_offstreet: MultiMap<LaneID, (BuildingID, Distance)>,

    // Parking lots
    num_spots_per_lot: BTreeMap<ParkingLotID, usize>,
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    driving_to_lots: MultiMap<LaneID, ParkingLotID>,

    events: Vec<Event>,
}

impl NormalParkingSimState {
    fn new(map: &Map, timer: &mut Timer) -> NormalParkingSimState {
        let mut sim = NormalParkingSimState {
            parked_cars: BTreeMap::new(),
            occupants: BTreeMap::new(),
            reserved_spots: BTreeSet::new(),

            onstreet_lanes: BTreeMap::new(),
            driving_to_parking_lanes: MultiMap::new(),
            num_spots_per_offstreet: BTreeMap::new(),
            driving_to_offstreet: MultiMap::new(),
            num_spots_per_lot: BTreeMap::new(),
            driving_to_lots: MultiMap::new(),

            events: Vec::new(),
        };
        for l in map.all_lanes() {
            if let Some(lane) = ParkingLane::new(l, map, timer) {
                sim.driving_to_parking_lanes.insert(lane.driving_lane, l.id);
                sim.onstreet_lanes.insert(lane.parking_lane, lane);
            }
        }
        for b in map.all_buildings() {
            if let Some((pos, _)) = b.driving_connection(map) {
                if !map.get_l(pos.lane()).driving_blackhole {
                    let num_spots = b.num_parking_spots();
                    if num_spots > 0 {
                        sim.num_spots_per_offstreet.insert(b.id, num_spots);
                        sim.driving_to_offstreet
                            .insert(pos.lane(), (b.id, pos.dist_along()));
                    }
                }
            }
        }
        for pl in map.all_parking_lots() {
            if !map.get_l(pl.driving_pos.lane()).driving_blackhole {
                sim.num_spots_per_lot.insert(pl.id, pl.capacity());
                sim.driving_to_lots.insert(pl.driving_pos.lane(), pl.id);
            }
        }

        sim
    }
}

impl ParkingSim for NormalParkingSimState {
    fn handle_live_edits(&mut self, map: &Map, timer: &mut Timer) -> Vec<ParkedCar> {
        let (filled_before, _) = self.get_all_parking_spots();
        let new = NormalParkingSimState::new(map, timer);
        let (_, avail_after) = new.get_all_parking_spots();
        let avail_after: BTreeSet<ParkingSpot> = avail_after.into_iter().collect();

        // Use the new spots
        self.onstreet_lanes = new.onstreet_lanes;
        self.driving_to_parking_lanes = new.driving_to_parking_lanes;
        self.num_spots_per_offstreet = new.num_spots_per_offstreet;
        self.driving_to_offstreet = new.driving_to_offstreet;
        self.num_spots_per_lot = new.num_spots_per_lot;
        self.driving_to_lots = new.driving_to_lots;

        // For every spot filled or reserved before, make sure that same spot still exists. If not,
        // evict that car.
        let mut evicted = Vec::new();
        for spot in filled_before {
            if !avail_after.contains(&spot) {
                let car = self.occupants.remove(&spot).unwrap();
                evicted.push(self.parked_cars.remove(&car).unwrap());
            }
        }

        // TODO How do we handle reserved_spots?
        self.reserved_spots = self
            .reserved_spots
            .difference(&avail_after)
            .cloned()
            .collect();

        evicted
    }

    fn get_free_onstreet_spots(&self, l: LaneID) -> Vec<ParkingSpot> {
        let mut spots: Vec<ParkingSpot> = Vec::new();
        if let Some(lane) = self.onstreet_lanes.get(&l) {
            for spot in lane.spots() {
                if self.is_free(spot) {
                    spots.push(spot);
                }
            }
        }
        spots
    }

    fn get_free_offstreet_spots(&self, b: BuildingID) -> Vec<ParkingSpot> {
        let mut spots: Vec<ParkingSpot> = Vec::new();
        for idx in 0..self.num_spots_per_offstreet.get(&b).cloned().unwrap_or(0) {
            let spot = ParkingSpot::Offstreet(b, idx);
            if self.is_free(spot) {
                spots.push(spot);
            }
        }
        spots
    }

    fn get_free_lot_spots(&self, pl: ParkingLotID) -> Vec<ParkingSpot> {
        let mut spots: Vec<ParkingSpot> = Vec::new();
        for idx in 0..self.num_spots_per_lot.get(&pl).cloned().unwrap_or(0) {
            let spot = ParkingSpot::Lot(pl, idx);
            if self.is_free(spot) {
                spots.push(spot);
            }
        }
        spots
    }

    fn reserve_spot(&mut self, spot: ParkingSpot) {
        assert!(self.is_free(spot));
        self.reserved_spots.insert(spot);

        // Sanity check the spot exists
        match spot {
            ParkingSpot::Onstreet(l, idx) => {
                assert!(idx < self.onstreet_lanes[&l].spot_dist_along.len());
            }
            ParkingSpot::Offstreet(b, idx) => {
                assert!(idx < self.num_spots_per_offstreet[&b]);
            }
            ParkingSpot::Lot(pl, idx) => {
                assert!(idx < self.num_spots_per_lot[&pl]);
            }
        }
    }

    fn remove_parked_car(&mut self, p: ParkedCar) {
        self.parked_cars
            .remove(&p.vehicle.id)
            .expect("remove_parked_car missing from parked_cars");
        self.occupants
            .remove(&p.spot)
            .expect("remove_parked_car missing from occupants");
        self.events
            .push(Event::CarLeftParkingSpot(p.vehicle.id, p.spot));
    }

    fn add_parked_car(&mut self, p: ParkedCar) {
        self.events
            .push(Event::CarReachedParkingSpot(p.vehicle.id, p.spot));

        assert!(self.reserved_spots.remove(&p.spot));

        assert!(!self.occupants.contains_key(&p.spot));
        self.occupants.insert(p.spot, p.vehicle.id);

        assert!(!self.parked_cars.contains_key(&p.vehicle.id));
        self.parked_cars.insert(p.vehicle.id, p);
    }

    fn get_draw_cars(&self, id: LaneID, map: &Map) -> Vec<DrawCarInput> {
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

    fn get_draw_cars_in_lots(&self, id: LaneID, map: &Map) -> Vec<DrawCarInput> {
        let mut cars = Vec::new();
        for pl in self.driving_to_lots.get(id) {
            for idx in 0..self.num_spots_per_lot[&pl] {
                if let Some(car) = self.occupants.get(&ParkingSpot::Lot(*pl, idx)) {
                    if let Some(d) = self.get_draw_car(*car, map) {
                        cars.push(d);
                    }
                }
            }
        }
        cars
    }

    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput> {
        let p = self.parked_cars.get(&id)?;
        match p.spot {
            ParkingSpot::Onstreet(lane, idx) => {
                let front_dist = self.onstreet_lanes[&lane].dist_along_for_car(idx, &p.vehicle);
                Some(DrawCarInput {
                    id: p.vehicle.id,
                    waiting_for_turn: None,
                    status: CarStatus::Parked,
                    on: Traversable::Lane(lane),
                    partly_on: Vec::new(),
                    label: None,

                    body: map
                        .get_l(lane)
                        .lane_center_pts
                        .exact_slice(front_dist - p.vehicle.length, front_dist),
                })
            }
            ParkingSpot::Offstreet(_, _) => None,
            ParkingSpot::Lot(pl, idx) => {
                let pl = map.get_pl(pl);
                // Some cars might be in the unrenderable extra_spots.
                let (pt, angle) = pl.spots.get(idx)?;
                let buffer = Distance::meters(0.5);
                Some(DrawCarInput {
                    id: p.vehicle.id,
                    waiting_for_turn: None,
                    status: CarStatus::Parked,
                    // Just used for z-order
                    on: Traversable::Lane(pl.driving_pos.lane()),
                    partly_on: Vec::new(),
                    label: None,

                    body: PolyLine::must_new(vec![
                        pt.project_away(buffer, *angle),
                        pt.project_away(map_model::PARKING_LOT_SPOT_LENGTH - buffer, *angle),
                    ]),
                })
            }
        }
    }

    fn canonical_pt(&self, id: CarID, map: &Map) -> Option<Pt2D> {
        let p = self.parked_cars.get(&id)?;
        match p.spot {
            ParkingSpot::Onstreet(_, _) => Some(self.get_draw_car(id, map).unwrap().body.last_pt()),
            ParkingSpot::Lot(pl, _) => {
                if let Some(car) = self.get_draw_car(id, map) {
                    Some(car.body.last_pt())
                } else {
                    Some(map.get_pl(pl).polygon.center())
                }
            }
            ParkingSpot::Offstreet(b, _) => Some(map.get_b(b).label_center),
        }
    }

    fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput> {
        self.parked_cars
            .keys()
            .filter_map(|id| self.get_draw_car(*id, map))
            .collect()
    }

    fn is_free(&self, spot: ParkingSpot) -> bool {
        !self.occupants.contains_key(&spot) && !self.reserved_spots.contains(&spot)
    }

    fn get_car_at_spot(&self, spot: ParkingSpot) -> Option<&ParkedCar> {
        let car = self.occupants.get(&spot)?;
        Some(&self.parked_cars[&car])
    }

    fn get_all_free_spots(
        &self,
        driving_pos: Position,
        vehicle: &Vehicle,
        // Either the building where a seeded car starts or the target of a trip. For filtering
        // private spots.
        target: BuildingID,
        map: &Map,
    ) -> Vec<(ParkingSpot, Position)> {
        let mut candidates = Vec::new();

        for l in self.driving_to_parking_lanes.get(driving_pos.lane()) {
            for spot in self.onstreet_lanes[l].spots() {
                if self.is_free(spot)
                    && driving_pos.dist_along()
                        < self.spot_to_driving_pos(spot, vehicle, map).dist_along()
                {
                    candidates.push(spot);
                }
            }
        }

        for (b, bldg_dist) in self.driving_to_offstreet.get(driving_pos.lane()) {
            if let OffstreetParking::Private(_, _) = map.get_b(*b).parking {
                if target != *b {
                    continue;
                }
            }
            if driving_pos.dist_along() < *bldg_dist {
                for idx in 0..self.num_spots_per_offstreet[b] {
                    let spot = ParkingSpot::Offstreet(*b, idx);
                    if self.is_free(spot) {
                        candidates.push(spot);
                    }
                }
            }
        }

        for pl in self.driving_to_lots.get(driving_pos.lane()) {
            let lot_dist = map.get_pl(*pl).driving_pos.dist_along();
            if driving_pos.dist_along() < lot_dist {
                for idx in 0..self.num_spots_per_lot[&pl] {
                    let spot = ParkingSpot::Lot(*pl, idx);
                    if self.is_free(spot) {
                        candidates.push(spot);
                    }
                }
            }
        }

        candidates
            .into_iter()
            .map(|spot| (spot, self.spot_to_driving_pos(spot, vehicle, map)))
            .collect()
    }

    fn spot_to_driving_pos(&self, spot: ParkingSpot, vehicle: &Vehicle, map: &Map) -> Position {
        match spot {
            ParkingSpot::Onstreet(l, idx) => {
                let lane = &self.onstreet_lanes[&l];
                Position::new(l, lane.dist_along_for_car(idx, vehicle)).equiv_pos_for_long_object(
                    lane.driving_lane,
                    vehicle.length,
                    map,
                )
            }
            ParkingSpot::Offstreet(b, _) => map.get_b(b).driving_connection(map).unwrap().0,
            ParkingSpot::Lot(pl, _) => map.get_pl(pl).driving_pos,
        }
    }

    fn spot_to_sidewalk_pos(&self, spot: ParkingSpot, map: &Map) -> Position {
        match spot {
            ParkingSpot::Onstreet(l, idx) => {
                let lane = &self.onstreet_lanes[&l];
                // Always centered in the entire parking spot
                Position::new(
                    l,
                    lane.spot_dist_along[idx] - (map_model::PARKING_SPOT_LENGTH / 2.0),
                )
                .equiv_pos(lane.sidewalk, map)
            }
            ParkingSpot::Offstreet(b, _) => map.get_b(b).sidewalk_pos,
            ParkingSpot::Lot(pl, _) => map.get_pl(pl).sidewalk_pos,
        }
    }

    fn get_owner_of_car(&self, id: CarID) -> Option<PersonID> {
        self.parked_cars.get(&id).and_then(|p| p.vehicle.owner)
    }
    fn lookup_parked_car(&self, id: CarID) -> Option<&ParkedCar> {
        self.parked_cars.get(&id)
    }

    fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        let mut spots = Vec::new();
        for lane in self.onstreet_lanes.values() {
            spots.extend(lane.spots());
        }
        for (b, num_spots) in &self.num_spots_per_offstreet {
            for idx in 0..*num_spots {
                spots.push(ParkingSpot::Offstreet(*b, idx));
            }
        }
        for (pl, num_spots) in &self.num_spots_per_lot {
            for idx in 0..*num_spots {
                spots.push(ParkingSpot::Lot(*pl, idx));
            }
        }

        let mut filled = Vec::new();
        let mut available = Vec::new();
        for spot in spots {
            if self.is_free(spot) {
                available.push(spot);
            } else {
                filled.push(spot);
            }
        }
        (filled, available)
    }

    fn path_to_free_parking_spot(
        &self,
        start: LaneID,
        vehicle: &Vehicle,
        target: BuildingID,
        map: &Map,
    ) -> Option<(Vec<PathStep>, ParkingSpot, Position)> {
        let mut backrefs: HashMap<LaneID, TurnID> = HashMap::new();
        // Don't travel far.
        // This is a max-heap, so negate all distances. Tie breaker is lane ID, arbitrary but
        // deterministic.
        let mut queue: BinaryHeap<(Distance, LaneID)> = BinaryHeap::new();
        queue.push((Distance::ZERO, start));

        while !queue.is_empty() {
            let (dist_so_far, current) = queue.pop().unwrap();
            // If the current lane has a spot open, we wouldn't be asking. This can happen if a spot
            // opens up on the 'start' lane, but behind the car.
            if current != start {
                // Pick the closest to the start of the lane, since that's closest to where we came
                // from
                if let Some((spot, pos)) = self
                    .get_all_free_spots(Position::start(current), vehicle, target, map)
                    .into_iter()
                    .min_by_key(|(_, pos)| pos.dist_along())
                {
                    let mut steps = vec![PathStep::Lane(current)];
                    let mut current = current;
                    loop {
                        if current == start {
                            // Don't include PathStep::Lane(start)
                            steps.pop();
                            steps.reverse();
                            return Some((steps, spot, pos));
                        }
                        let turn = backrefs[&current];
                        steps.push(PathStep::Turn(turn));
                        steps.push(PathStep::Lane(turn.src));
                        current = turn.src;
                    }
                }
            }
            for turn in map.get_turns_for(current, PathConstraints::Car) {
                if !backrefs.contains_key(&turn.id.dst) {
                    let dist_this_step = turn.geom.length() + map.get_l(current).length();
                    backrefs.insert(turn.id.dst, turn.id);
                    // Remember, keep things negative
                    queue.push((dist_so_far - dist_this_step, turn.id.dst));
                }
            }
        }

        None
    }

    fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    fn all_parked_car_positions(&self, map: &Map) -> Vec<(Position, PersonID)> {
        self.parked_cars
            .values()
            .map(|p| {
                (
                    self.spot_to_sidewalk_pos(p.spot, map),
                    p.vehicle.owner.unwrap(),
                )
            })
            .collect()
    }

    fn bldg_to_parked_cars(&self, b: BuildingID) -> Vec<CarID> {
        let mut cars = Vec::new();
        for idx in 0..self.num_spots_per_offstreet.get(&b).cloned().unwrap_or(0) {
            let spot = ParkingSpot::Offstreet(b, idx);
            if let Some(car) = self.occupants.get(&spot) {
                cars.push(*car);
            }
        }
        cars
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct ParkingLane {
    parking_lane: LaneID,
    driving_lane: LaneID,
    sidewalk: LaneID,
    // The front of the parking spot (farthest along the lane)
    spot_dist_along: Vec<Distance>,
}

impl ParkingLane {
    fn new(lane: &Lane, map: &Map, timer: &mut Timer) -> Option<ParkingLane> {
        if lane.lane_type != LaneType::Parking {
            return None;
        }

        let driving_lane = if let Some(l) = map.get_parent(lane.id).parking_to_driving(lane.id, map)
        {
            l
        } else {
            // Serious enough to blow up loudly.
            panic!("Parking lane {} has no driving lane!", lane.id);
        };
        if map.get_l(driving_lane).driving_blackhole {
            return None;
        }
        let sidewalk = if let Some(l) =
            map.get_parent(lane.id)
                .find_closest_lane(lane.id, |l| l.is_walkable(), map)
        {
            l
        } else {
            timer.warn(format!("Parking lane {} has no sidewalk!", lane.id));
            return None;
        };

        Some(ParkingLane {
            parking_lane: lane.id,
            driving_lane,
            sidewalk,
            spot_dist_along: (0..lane.number_parking_spots())
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
            spots.push(ParkingSpot::Onstreet(self.parking_lane, idx));
        }
        spots
    }
}

// This assigns infinite private parking to all buildings and none anywhere else. This effectively
// disables the simulation of parking entirely, making driving trips just go directly between
// buildings. Useful for maps without good parking data (which is currently all of them) and
// experiments where parking contention skews results and just gets in the way.
#[derive(Serialize, Deserialize, Clone)]
pub struct InfiniteParkingSimState {
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

    // Cache dist_along
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    driving_to_offstreet: MultiMap<LaneID, (BuildingID, Distance)>,
    blackholed_buildings: BTreeSet<BuildingID>,

    events: Vec<Event>,
}

impl InfiniteParkingSimState {
    fn new(map: &Map) -> InfiniteParkingSimState {
        let mut sim = InfiniteParkingSimState {
            parked_cars: BTreeMap::new(),
            occupants: BTreeMap::new(),
            reserved_spots: BTreeSet::new(),

            driving_to_offstreet: MultiMap::new(),
            blackholed_buildings: BTreeSet::new(),

            events: Vec::new(),
        };
        for b in map.all_buildings() {
            if let Some((pos, _)) = b.driving_connection(map) {
                if !map.get_l(pos.lane()).driving_blackhole {
                    sim.driving_to_offstreet
                        .insert(pos.lane(), (b.id, pos.dist_along()));
                    continue;
                }
            }
            sim.blackholed_buildings.insert(b.id);
        }
        sim
    }

    fn get_free_bldg_spot(&self, b: BuildingID) -> ParkingSpot {
        assert!(!self.blackholed_buildings.contains(&b));
        let mut i = 0;
        loop {
            let spot = ParkingSpot::Offstreet(b, i);
            if self.is_free(spot) {
                return spot;
            }
            i += 1;
        }
    }
}

impl ParkingSim for InfiniteParkingSimState {
    fn handle_live_edits(&mut self, map: &Map, _: &mut Timer) -> Vec<ParkedCar> {
        // Can live edits possibly affect anything?
        let new = InfiniteParkingSimState::new(map);
        self.driving_to_offstreet = new.driving_to_offstreet;
        self.blackholed_buildings = new.blackholed_buildings;

        Vec::new()
    }

    fn get_free_onstreet_spots(&self, _: LaneID) -> Vec<ParkingSpot> {
        Vec::new()
    }

    fn get_free_offstreet_spots(&self, b: BuildingID) -> Vec<ParkingSpot> {
        if self.blackholed_buildings.contains(&b) {
            Vec::new()
        } else {
            // Just returns the next free spot
            vec![self.get_free_bldg_spot(b)]
        }
    }

    fn get_free_lot_spots(&self, _: ParkingLotID) -> Vec<ParkingSpot> {
        Vec::new()
    }

    fn reserve_spot(&mut self, spot: ParkingSpot) {
        assert!(self.is_free(spot));
        self.reserved_spots.insert(spot);
    }

    fn remove_parked_car(&mut self, p: ParkedCar) {
        self.parked_cars
            .remove(&p.vehicle.id)
            .expect("remove_parked_car missing from parked_cars");
        self.occupants
            .remove(&p.spot)
            .expect("remove_parked_car missing from occupants");
        self.events
            .push(Event::CarLeftParkingSpot(p.vehicle.id, p.spot));
    }

    fn add_parked_car(&mut self, p: ParkedCar) {
        self.events
            .push(Event::CarReachedParkingSpot(p.vehicle.id, p.spot));

        assert!(self.reserved_spots.remove(&p.spot));

        assert!(!self.occupants.contains_key(&p.spot));
        self.occupants.insert(p.spot, p.vehicle.id);

        assert!(!self.parked_cars.contains_key(&p.vehicle.id));
        self.parked_cars.insert(p.vehicle.id, p);
    }

    fn get_draw_cars(&self, _: LaneID, _: &Map) -> Vec<DrawCarInput> {
        Vec::new()
    }

    fn get_draw_cars_in_lots(&self, _: LaneID, _: &Map) -> Vec<DrawCarInput> {
        Vec::new()
    }

    fn get_draw_car(&self, _: CarID, _: &Map) -> Option<DrawCarInput> {
        None
    }

    fn canonical_pt(&self, id: CarID, map: &Map) -> Option<Pt2D> {
        let p = self.parked_cars.get(&id)?;
        match p.spot {
            ParkingSpot::Offstreet(b, _) => Some(map.get_b(b).label_center),
            _ => unreachable!(),
        }
    }

    fn get_all_draw_cars(&self, _: &Map) -> Vec<DrawCarInput> {
        Vec::new()
    }

    fn is_free(&self, spot: ParkingSpot) -> bool {
        !self.occupants.contains_key(&spot) && !self.reserved_spots.contains(&spot)
    }

    fn get_car_at_spot(&self, spot: ParkingSpot) -> Option<&ParkedCar> {
        let car = self.occupants.get(&spot)?;
        Some(&self.parked_cars[&car])
    }

    fn get_all_free_spots(
        &self,
        driving_pos: Position,
        vehicle: &Vehicle,
        target: BuildingID,
        map: &Map,
    ) -> Vec<(ParkingSpot, Position)> {
        // The target building may be blackholed, so fallback to a building on one of the
        // penultimate lanes, when the search begins.
        let mut bldg: Option<BuildingID> = None;
        for (b, bldg_dist) in self.driving_to_offstreet.get(driving_pos.lane()) {
            if driving_pos.dist_along() >= *bldg_dist {
                continue;
            }
            if target == *b {
                bldg = Some(target);
                break;
            } else if bldg.is_none() {
                // Backup option
                bldg = Some(*b);
            }
        }
        if let Some(b) = bldg {
            let spot = self.get_free_bldg_spot(b);
            vec![(spot, self.spot_to_driving_pos(spot, vehicle, map))]
        } else {
            Vec::new()
        }
    }

    fn spot_to_driving_pos(&self, spot: ParkingSpot, _: &Vehicle, map: &Map) -> Position {
        match spot {
            ParkingSpot::Offstreet(b, _) => map.get_b(b).driving_connection(map).unwrap().0,
            _ => unreachable!(),
        }
    }

    fn spot_to_sidewalk_pos(&self, spot: ParkingSpot, map: &Map) -> Position {
        match spot {
            ParkingSpot::Offstreet(b, _) => map.get_b(b).sidewalk_pos,
            _ => unreachable!(),
        }
    }

    fn get_owner_of_car(&self, id: CarID) -> Option<PersonID> {
        self.parked_cars.get(&id).and_then(|p| p.vehicle.owner)
    }
    fn lookup_parked_car(&self, id: CarID) -> Option<&ParkedCar> {
        self.parked_cars.get(&id)
    }

    fn get_all_parking_spots(&self) -> (Vec<ParkingSpot>, Vec<ParkingSpot>) {
        unreachable!()
    }

    fn path_to_free_parking_spot(
        &self,
        _: LaneID,
        _: &Vehicle,
        _: BuildingID,
        _: &Map,
    ) -> Option<(Vec<PathStep>, ParkingSpot, Position)> {
        // The original building we're aiming for will always have room, unless it's located on a
        // blackholed lane. In that case, there's usually a nearby building on the last connected
        // lane. If not, then just give up for now.
        None
    }

    fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    fn all_parked_car_positions(&self, map: &Map) -> Vec<(Position, PersonID)> {
        self.parked_cars
            .values()
            .map(|p| {
                (
                    self.spot_to_sidewalk_pos(p.spot, map),
                    p.vehicle.owner.unwrap(),
                )
            })
            .collect()
    }

    fn bldg_to_parked_cars(&self, b: BuildingID) -> Vec<CarID> {
        // TODO This is a very inefficient impl
        let mut cars = Vec::new();
        for (spot, car) in &self.occupants {
            if let ParkingSpot::Offstreet(bldg, _) = spot {
                if b == *bldg {
                    cars.push(*car);
                }
            }
        }
        cars
    }
}

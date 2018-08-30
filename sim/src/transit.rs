use map_model::BusStop;
use std::collections::BTreeMap;
use CarID;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum BusState {
    Driving,
    AtStop(BusStop),
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitSimState {
    buses: BTreeMap<CarID, BusState>,
}

impl TransitSimState {
    pub fn new() -> TransitSimState {
        TransitSimState {
            buses: BTreeMap::new(),
        }
    }

    // Transitions
    pub fn bus_is_driving(&mut self, bus: CarID) {
        self.buses.insert(bus, BusState::Driving);
    }

    pub fn bus_is_at_stop(&mut self, bus: CarID, stop: BusStop) {
        self.buses.insert(bus, BusState::AtStop(stop));
    }
}

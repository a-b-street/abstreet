use CarID;
use map_model::{Road, RoadID};
use std::iter;

struct ParkingLane {
    r: RoadID,
    spots: Vec<Option<CarID>>,
}

impl ParkingLane {
    fn new(r: &Road) -> ParkingLane {
        ParkingLane {
            r: r.id,
            spots: iter::repeat(None).take(r.number_parking_spots()).collect(),
        }
    }
}

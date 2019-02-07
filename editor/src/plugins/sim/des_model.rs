use geom::{Acceleration, Distance, Duration, Speed};
use map_model::{LaneID, Map, Traversable};
use sim::{CarID, CarState, DrawCarInput, VehicleType};

pub fn get_state(time: Duration, map: &Map) -> Vec<DrawCarInput> {
    let lane = map.get_l(LaneID(1250));
    let leader = CarID::tmp_new(0, VehicleType::Car);
    let speed = map.get_parent(lane.id).get_speed_limit();
    let car_length = Distance::meters(5.0);

    vec![draw_car(leader, car_length + speed * time, map)]
}

fn draw_car(id: CarID, front: Distance, map: &Map) -> DrawCarInput {
    let lane = map.get_l(LaneID(1250));
    let car_length = Distance::meters(5.0);

    DrawCarInput {
        id,
        waiting_for_turn: None,
        stopping_trace: None,
        state: CarState::Moving,
        vehicle_type: VehicleType::Car,
        on: Traversable::Lane(lane.id),
        body: lane
            .lane_center_pts
            .slice(front - car_length, front)
            .unwrap()
            .0,
    }
}

struct Interval {
    start_dist: Distance,
    end_dist: Distance,
    start_time: Duration,
    end_time: Duration,
    start_speed: Speed,
    end_speed: Speed,
    // Extra info: CarID, LaneID
}

impl Interval {
    // TODO validate in the constructor

    fn dist(&self, t: Duration) -> Distance {
        // Linearly interpolate
        self.percent(t) * (self.end_dist - self.start_dist)
    }

    fn speed(&self, t: Duration) -> Speed {
        // Linearly interpolate
        self.percent(t) * (self.end_speed - self.start_speed)
    }

    fn percent(&self, t: Duration) -> f64 {
        assert!(t >= self.start_time);
        assert!(t <= self.end_time);
        (t - self.start_time) / (self.end_time - self.start_time)
    }

    // TODO intersection operation figures out dist and speed by interpolating, of course.
}

// TODO construct Interval for starting from rest, for stopping, for constant distance travel
// TODO make a car follow intervals in sequence
// TODO use lane length and a car's properties to figure out reasonable intervals for short/long
// lanes

struct Car {
    id: CarID,
    max_accel: Acceleration,
    max_deaccel: Acceleration,
    intervals: Vec<Interval>,
}

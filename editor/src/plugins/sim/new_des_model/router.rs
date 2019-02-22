use geom::Distance;
use map_model::{Map, Traversable};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Router {
    // Front is always the current step
    path: VecDeque<Traversable>,
    end_dist: Distance,
}

impl Router {
    pub fn stop_suddenly(path: Vec<Traversable>, end_dist: Distance, map: &Map) -> Router {
        if end_dist >= path.last().unwrap().length(map) {
            panic!(
                "Can't end a car at {}; {:?} isn't that long",
                end_dist,
                path.last().unwrap()
            );
        }

        Router {
            path: VecDeque::from(path),
            end_dist,
        }
    }

    pub fn validate_start_dist(&self, start_dist: Distance) {
        if self.path.len() == 1 && start_dist >= self.end_dist {
            panic!(
                "Can't start a car with one path in its step and go from {} to {}",
                start_dist, self.end_dist
            );
        }
    }

    pub fn head(&self) -> Traversable {
        self.path[0]
    }

    pub fn next(&self) -> Traversable {
        self.path[1]
    }

    pub fn last_step(&self) -> bool {
        self.path.len() == 1
    }

    pub fn get_end_dist(&self) -> Distance {
        // Shouldn't ask earlier!
        assert!(self.last_step());
        self.end_dist
    }

    // Returns the step just finished
    pub fn advance(&mut self) -> Traversable {
        self.path.pop_front().unwrap()
    }

    // Called when the car is Queued at the last step. Returns true if the car is completely done!
    pub fn maybe_handle_end(&mut self, front: Distance) -> bool {
        // TODO Could do some replanning here for parking, for example
        self.end_dist == front
    }
}

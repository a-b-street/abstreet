use rand::distributions::{Distribution, Weighted, WeightedChoice};
use rand::{RngCore, SeedableRng, XorShiftRng};
use serde_derive::{Deserialize, Serialize};

// Need to explain this trick -- basically keeps consistency between two different simulations when
// each one might make slightly different sequences of calls to the RNG.
pub fn fork_rng(base_rng: &mut XorShiftRng) -> XorShiftRng {
    XorShiftRng::from_seed([base_rng.next_u32() as u8; 16])
}

// Represents the probability of sampling 0, 1, 2, 3... The sum can be anything.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightedUsizeChoice {
    pub weights: Vec<u32>,
}

impl WeightedUsizeChoice {
    pub fn parse(string: &str) -> Option<WeightedUsizeChoice> {
        let parts: Vec<&str> = string.split(",").collect();
        if parts.is_empty() {
            return None;
        }
        let mut weights: Vec<u32> = Vec::new();
        for x in parts.into_iter() {
            let x = x.parse::<u32>().ok()?;
            weights.push(x);
        }
        Some(WeightedUsizeChoice { weights })
    }

    pub fn sample(&self, rng: &mut XorShiftRng) -> u32 {
        let mut items: Vec<Weighted<u32>> = self
            .weights
            .iter()
            .enumerate()
            .map(|(idx, pr)| Weighted {
                weight: *pr,
                item: idx as u32,
            })
            .collect();
        WeightedChoice::new(&mut items).sample(rng)
    }
}

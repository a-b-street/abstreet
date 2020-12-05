use std::collections::HashMap;

use crate::levels::Level;

/// Persistent state that lasts across levels.
// TODO Save and load it!
pub struct Session {
    pub levels: Vec<Level>,
    /// Level title -> the top 3 scores
    pub high_scores: HashMap<&'static str, Vec<usize>>,
    pub levels_unlocked: usize,
    pub current_vehicle: &'static str,
    pub vehicles_unlocked: Vec<&'static str>,
    pub upzones_unlocked: usize,
}

impl Session {
    pub fn new() -> Session {
        let levels = Level::all();
        let mut high_scores = HashMap::new();
        for level in &levels {
            high_scores.insert(level.title, Vec::new());
        }
        Session {
            levels,
            high_scores,
            levels_unlocked: 1,
            current_vehicle: "sleigh",
            vehicles_unlocked: vec!["sleigh"],
            upzones_unlocked: 0,
        }
    }

    /// If a message is returned, a new level and some powers were unlocked.
    pub fn record_score(&mut self, level: &'static str, score: usize) -> Option<Vec<String>> {
        let scores = self.high_scores.get_mut(level).unwrap();
        scores.push(score);
        scores.sort();
        scores.reverse();
        scores.truncate(3);

        let idx = self
            .levels
            .iter()
            .position(|lvl| lvl.title == level)
            .unwrap();
        let level = &self.levels[idx];
        if idx + 1 == self.levels_unlocked && score >= level.goal {
            if idx + 1 == self.levels.len() {
                Some(vec![
                    format!("All levels complete! Nice."),
                    format!("Can you improve your score on other levels?"),
                ])
            } else {
                self.levels_unlocked += 1;
                let mut messages = vec![format!("New level unlocked!")];
                if level.unlock_upzones > 0 {
                    self.upzones_unlocked += level.unlock_upzones;
                    messages.push(format!(
                        "Unlocked the ability to upzone {} buildings",
                        level.unlock_upzones
                    ));
                }
                for x in &level.unlock_vehicles {
                    self.vehicles_unlocked.push(*x);
                    messages.push(format!("Unlocked the {}", x));
                }
                Some(messages)
            }
        } else {
            // Nothing new unlocked
            None
        }
    }

    pub fn unlock_all(&mut self) {
        for level in &self.levels {
            self.vehicles_unlocked.extend(level.unlock_vehicles.clone());
            self.upzones_unlocked += level.unlock_upzones;
        }
        self.levels_unlocked = self.levels.len();
    }
}

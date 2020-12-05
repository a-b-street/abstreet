use std::collections::HashMap;

use crate::levels::Level;

/// Persistent state that lasts across levels.
// TODO Save and load it!
pub struct Session {
    pub levels: Vec<Level>,
    /// Level title -> the top 3 scores
    pub high_scores: HashMap<&'static str, Vec<usize>>,
    pub levels_unlocked: usize,
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
        }
    }

    /// If a message is returned, a new level and some powers were unlocked.
    pub fn record_score(&mut self, level: &'static str, score: usize) -> Option<String> {
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
        if idx + 1 == self.levels_unlocked && score >= self.levels[idx].goal {
            if idx + 1 == self.levels.len() {
                Some(format!("All levels complete! Nice."))
            } else {
                self.levels_unlocked += 1;
                Some(format!("New level unlocked!"))
            }
        } else {
            // Nothing new unlocked
            None
        }
    }
}

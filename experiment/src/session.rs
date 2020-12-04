use std::collections::HashMap;

use crate::levels::Level;

/// Persistent state that lasts across levels.
// TODO Save and load it!
#[derive(Clone)]
pub struct Session {
    // Level title -> the top 3 scores
    pub high_scores: HashMap<&'static str, Vec<usize>>,
}

impl Session {
    pub fn new() -> Session {
        let mut high_scores = HashMap::new();
        for level in Level::all() {
            high_scores.insert(level.title, Vec::new());
        }
        Session { high_scores }
    }

    pub fn record_score(&mut self, level: &'static str, score: usize) {
        let scores = self.high_scores.get_mut(level).unwrap();
        scores.push(score);
        scores.sort();
        scores.reverse();
        scores.truncate(3);
    }
}

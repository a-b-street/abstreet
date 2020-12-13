use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use widgetry::{Color, EventCtx};

use crate::levels::Level;
use crate::music::Music;

/// Persistent state that lasts across levels.
#[derive(Serialize, Deserialize)]
pub struct Session {
    // It's convenient to also serialize these, to tune the game without recompiling.
    pub levels: Vec<Level>,
    pub colors: ColorScheme,

    /// Level title -> the top 3 scores
    pub high_scores: HashMap<String, Vec<usize>>,
    pub levels_unlocked: usize,
    pub current_vehicle: String,
    pub vehicles_unlocked: BTreeSet<String>,
    pub upzones_unlocked: usize,

    #[serde(skip_serializing, skip_deserializing)]
    pub music: Music,
    pub play_music: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ColorScheme {
    pub house: Color,
    pub apartment: Color,
    pub store: Color,
    pub visited: Color,

    pub score: Color,
    pub energy: Color,
    pub boost: Color,
}

impl Session {
    pub fn load() -> Session {
        let levels = Level::all();

        if let Ok(session) = abstutil::maybe_read_json::<Session>(
            abstutil::path_player("santa.json"),
            &mut Timer::throwaway(),
        ) {
            // TODO Explicit version number to detect more easily?
            if session.levels == levels {
                return session;
            }
            // TODO Try to preserve high scores or levels unlocked? It could get complicated,
            // depending on how levels were changed or reordered.
            warn!("Loaded session data, but the levels have changed, so discarding!");
        }

        let mut high_scores = HashMap::new();
        for level in &levels {
            high_scores.insert(level.title.clone(), Vec::new());
        }
        Session {
            levels,
            colors: ColorScheme {
                house: Color::hex("#688865"),
                apartment: Color::hex("#C0F879"),
                store: Color::hex("#F4DF4D"),
                visited: Color::BLACK,

                score: Color::hex("#83AA51"),
                energy: Color::hex("#D8B830"),
                boost: Color::hex("#A32015"),
            },

            high_scores,
            levels_unlocked: 1,
            current_vehicle: "sleigh".to_string(),
            vehicles_unlocked: vec!["sleigh".to_string()].into_iter().collect(),
            upzones_unlocked: 0,

            music: Music::empty(),
            play_music: true,
        }
    }

    /// If a message is returned, a new level and some powers were unlocked.
    pub fn record_score(&mut self, level: String, score: usize) -> Option<Vec<String>> {
        let scores = self.high_scores.get_mut(&level).unwrap();
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
        let msg = if idx + 1 == self.levels_unlocked && score >= level.goal {
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
                    self.vehicles_unlocked.insert(x.clone());
                    messages.push(format!("Unlocked the {}", x));
                }
                Some(messages)
            }
        } else {
            // Nothing new unlocked
            None
        };
        abstutil::write_json(abstutil::path_player("santa.json"), self);
        msg
    }

    pub fn unlock_all(&mut self) {
        for level in &self.levels {
            self.vehicles_unlocked.extend(level.unlock_vehicles.clone());
            self.upzones_unlocked += level.unlock_upzones;
        }
        self.levels_unlocked = self.levels.len();
    }

    pub fn update_music(&mut self, ctx: &mut EventCtx) {
        let play_music = self.play_music;
        self.music.event(ctx, &mut self.play_music);
        if play_music != self.play_music {
            // Save when we mute/unmute
            abstutil::write_json(abstutil::path_player("santa.json"), self);
        }
    }
}

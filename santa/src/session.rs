use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_multimap, serialize_multimap, MultiMap, Timer};
use map_model::BuildingID;
use widgetry::{Color, EventCtx};

use crate::levels::Level;
use crate::music::Music;

/// Persistent state that lasts across levels.
#[derive(Serialize, Deserialize)]
pub struct Session {
    pub levels: Vec<Level>,
    /// Enable this to use the levels, instead of overwriting them with the version in the code.
    pub enable_modding: bool,
    pub colors: ColorScheme,

    /// Level title -> the top 3 scores
    pub high_scores: HashMap<String, Vec<usize>>,
    pub levels_unlocked: usize,
    pub current_vehicle: String,
    pub vehicles_unlocked: BTreeSet<String>,
    pub upzones_unlocked: usize,
    pub upzones_explained: bool,
    // This was added after the main release, so keep old save files working by allowing it to be
    // missing.
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap",
        default
    )]
    pub upzones_per_level: MultiMap<String, BuildingID>,

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

        if let Ok(mut session) = abstutil::maybe_read_json::<Session>(
            abstutil::path_player("santa.json"),
            &mut Timer::throwaway(),
        ) {
            if session.levels != levels {
                if session.enable_modding {
                    warn!("Using modified levels from the session data");
                } else {
                    warn!("Levels have changed; overwriting with the new version from the code");
                    session.levels = levels;
                }
            }
            return session;
        }

        let mut high_scores = HashMap::new();
        for level in &levels {
            high_scores.insert(level.title.clone(), Vec::new());
        }
        Session {
            levels,
            enable_modding: false,
            colors: ColorScheme {
                house: Color::hex("#688865"),
                apartment: Color::hex("#C0F879"),
                store: Color::hex("#EE702E"),
                visited: Color::BLACK,

                score: Color::hex("#83AA51"),
                energy: Color::hex("#D8B830"),
                boost: Color::hex("#A32015"),
            },

            high_scores,
            levels_unlocked: 1,
            current_vehicle: "bike".to_string(),
            vehicles_unlocked: vec!["bike".to_string()].into_iter().collect(),
            upzones_unlocked: 0,
            upzones_explained: false,
            upzones_per_level: MultiMap::new(),

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
        self.save();
        msg
    }

    pub fn unlock_all(&mut self) {
        self.upzones_unlocked = 0;
        for level in &self.levels {
            self.vehicles_unlocked.extend(level.unlock_vehicles.clone());
            self.upzones_unlocked += level.unlock_upzones;
        }
        self.levels_unlocked = self.levels.len();
        self.upzones_explained = true;
    }

    pub fn update_music(&mut self, ctx: &mut EventCtx) {
        let play_music = self.play_music;
        self.music.event(ctx, &mut self.play_music);
        if play_music != self.play_music {
            self.save();
        }
    }

    pub fn save(&self) {
        abstutil::write_json(abstutil::path_player("santa.json"), self);
    }
}

use abstutil;
use ezgui::Color;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::Error;

pub struct ColorScheme {
    // Filled out by lazy calls to get_def()
    map: HashMap<String, Color>,

    // A subset of map
    modified: ModifiedColors,
}

#[derive(Serialize, Deserialize)]
struct ModifiedColors {
    map: BTreeMap<String, Color>,
}

impl ColorScheme {
    pub fn load() -> Result<ColorScheme, Error> {
        let modified: ModifiedColors = abstutil::read_json("color_scheme")?;
        let mut map: HashMap<String, Color> = HashMap::new();
        for (name, c) in &modified.map {
            map.insert(name.clone(), *c);
        }

        Ok(ColorScheme { map, modified })
    }

    pub fn save(&self) {
        abstutil::write_json("color_scheme", &self.modified).expect("Saving color_scheme failed");
    }

    // Get, but specify the default inline
    pub fn get_def(&mut self, name: &str, default: Color) -> Color {
        if let Some(existing) = self.map.get(name) {
            if default != *existing && !self.modified.map.contains_key(name) {
                panic!(
                    "Two colors defined for {}: {} and {}",
                    name, existing, default
                );
            }
            return *existing;
        }

        self.map.insert(name.to_string(), default);
        default
    }

    // Just for the color picker plugin, that's why the funky return value
    pub fn color_names(&self) -> Vec<(String, ())> {
        let mut names: Vec<(String, ())> = self.map.keys().map(|n| (n.clone(), ())).collect();
        names.sort();
        names
    }

    pub fn override_color(&mut self, name: &str, value: Color) {
        self.modified.map.insert(name.to_string(), value);
        self.map.insert(name.to_string(), value);
    }

    pub fn get_modified(&self, name: &str) -> Option<Color> {
        self.modified.map.get(name).cloned()
    }

    pub fn reset_modified(&mut self, name: &str, orig: Option<Color>) {
        if let Some(c) = orig {
            self.modified.map.insert(name.to_string(), c);
            self.map.insert(name.to_string(), c);
        } else {
            self.modified.map.remove(name);
            // Just, uh, wait for the default to be populated next time. :P
            self.map.remove(name);
        }
    }
}

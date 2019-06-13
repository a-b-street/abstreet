use abstutil;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ABTest {
    pub test_name: String,

    pub map_name: String,
    pub scenario_name: String,
    pub edits1_name: String,
    pub edits2_name: String,
}

impl ABTest {
    pub fn describe(&self) -> Vec<String> {
        abstutil::to_json(self)
            .split('\n')
            .map(|s| s.to_string())
            .collect()
    }

    pub fn save(&self) {
        abstutil::save_json_object("ab_tests", &self.map_name, &self.test_name, self);
    }
}

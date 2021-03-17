use serde::Deserialize;
use serde_json;

#[derive(Deserialize)]
#[serde(default)]
pub struct ImporterConfiguration {
    pub osmconvert: String,
    pub unzip: String,
    pub gunzip: String,
    pub gunzip_args: String,
}

impl Default for ImporterConfiguration {
    fn default() -> ImporterConfiguration {
        ImporterConfiguration {
            osmconvert: String::from("osmconvert"),
            unzip: String::from("unzip"),
            gunzip: String::from("gunzip"),
            gunzip_args: String::from(""),
        }
    }
}

pub fn load_configuration() -> ImporterConfiguration {
    // Safe to assume that {} can be parsed given struct-level Default implementation.
    let default = serde_json::from_str("{}").unwrap();

    match std::fs::read_to_string("importer.json") {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or(default),
        Err(_) => default,
    }
}

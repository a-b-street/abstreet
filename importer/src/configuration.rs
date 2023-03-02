use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default)]
pub struct ImporterConfiguration {
    pub osmium: String,
    pub unzip: String,
    pub gunzip: String,
    pub gunzip_args: String,
}

impl Default for ImporterConfiguration {
    fn default() -> ImporterConfiguration {
        ImporterConfiguration {
            osmium: String::from("osmium"),
            unzip: String::from("unzip"),
            gunzip: String::from("gunzip"),
            gunzip_args: String::from(""),
        }
    }
}

impl ImporterConfiguration {
    pub fn load() -> Self {
        // Safe to assume that {} can be parsed given struct-level Default implementation.
        let default = serde_json::from_str("{}").unwrap();

        match fs_err::read_to_string("importer.json") {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or(default),
            Err(_) => default,
        }
    }
}

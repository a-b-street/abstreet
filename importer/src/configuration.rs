use serde::Deserialize;
use abstutil::maybe_read_json_without_timer;

#[derive(Deserialize)]
pub struct ImporterConfiguration {
    #[serde(default = "default_curl")]
    pub curl: String,
    #[serde(default = "default_osmconvert")]
    pub osmconvert: String,
    #[serde(default = "default_unzip")]
    pub unzip: String,
    #[serde(default = "default_gunzip")]
    pub gunzip: String,
    #[serde(default = "default_gunzip_args")]
    pub gunzip_args: Option<String>,
}

fn default_curl() -> String {
    String::from("curl")
}

fn default_osmconvert() -> String {
    String::from("osmconvert")
}

fn default_unzip() -> String {
    String::from("unzip")
}

fn default_gunzip() -> String {
    String::from("gunzip")
}

fn default_gunzip_args() -> Option<String> {
    None
}

pub fn load_configuration() -> ImporterConfiguration {

    match maybe_read_json_without_timer::<ImporterConfiguration>(&String::from("importer.json")) {
        Ok(config) => config,
        Err(_) => default_configuration(),
    }
}

fn default_configuration() -> ImporterConfiguration {
    ImporterConfiguration {
        curl: default_curl(),
        osmconvert: default_osmconvert(),
        unzip: default_unzip(),
        gunzip: default_gunzip(),
        gunzip_args: default_gunzip_args(),
    }
}
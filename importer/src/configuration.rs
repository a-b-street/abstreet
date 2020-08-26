use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
pub struct ImporterConfiguration {
    pub curl: String,
    pub osmconvert: String,
    pub unzip: String,
    pub gunzip: String,
    pub gunzip_args: Option<String>,
}

pub fn load_configuration() -> ImporterConfiguration {

    match fs::read_to_string("importer.toml") {
        Ok(text) => {
            match toml::from_str::<ImporterConfiguration>(&text[..]) {
                Ok(config) => config,
                Err(_) => default_configuration(),
            }
        },
        Err(_) => default_configuration(),
    }
}

fn default_configuration() -> ImporterConfiguration {
    ImporterConfiguration {
        curl: String::from("curl"),
        osmconvert: String::from("osmconvert"),
        unzip: String::from("unzip"),
        gunzip: String::from("gunzip"),
        gunzip_args: Option::None,
    }
}


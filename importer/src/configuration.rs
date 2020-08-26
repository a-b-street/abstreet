use serde::Deserialize;
use std::fs;

pub struct ImporterConfiguration {
    pub curl: String,
    pub osmconvert: String,
    pub unzip: String,
    pub gunzip: String,
    pub gunzip_args: Option<String>,
}

#[derive(Deserialize)]
struct RawImporterConfiguration {
    pub curl: Option<String>,
    pub osmconvert: Option<String>,
    pub unzip: Option<String>,
    pub gunzip: Option<String>,
    pub gunzip_args: Option<String>,
}

pub fn load_configuration() -> ImporterConfiguration {

    match fs::read_to_string("importer.toml") {
        Ok(text) => {
            match toml::from_str::<RawImporterConfiguration>(&text) {
                Ok(config) => fill_in_defaults(config),
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

fn fill_in_defaults(config: RawImporterConfiguration) -> ImporterConfiguration {
    let mut result = default_configuration();

    result.curl = value_or_default(config.curl, result.curl);
    result.osmconvert = value_or_default(config.osmconvert, result.osmconvert);
    result.unzip = value_or_default(config.unzip, result.unzip);
    result.gunzip = value_or_default(config.gunzip, result.gunzip);

    result.gunzip_args = config.gunzip_args;

    return result;
}

fn value_or_default<T>(maybe_value: Option<T>, default: T) -> T {
    if let Some(value) = maybe_value {
        return value;
    }
    return default;
}

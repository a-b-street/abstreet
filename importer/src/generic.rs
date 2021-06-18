use serde::{Deserialize, Serialize};

use abstio::MapName;
use map_model::raw::RawMap;

use crate::configuration::ImporterConfiguration;
use crate::utils::{download, osmconvert};

/// Importing a new city can be done just by filling out this config file and specifying some
/// polygon boundaries. Most fields are directly from `convert_osm::Options`.
///
/// If any extra data is imported for a city (like collisions or population), then for now, don't
/// use this.
#[derive(Serialize, Deserialize)]
pub struct GenericCityImporter {
    /// The URL to a .osm or .osm.pbf file containing the entire city.
    /// http://download.geofabrik.de/ is recommended.
    ///
    /// You can also put a path like `input/us/seattle/osm/washington-latest.osm.pbf` in here,
    /// and instead that file will be used. This is kind of a hack, because it'll assume the cities
    /// are imported in the proper order, but it prevents having to download duplicate large files.
    pub osm_url: String,

    pub map_config: map_model::MapConfig,
    pub onstreet_parking: convert_osm::OnstreetParking,
    pub public_offstreet_parking: convert_osm::PublicOffstreetParking,
    pub private_offstreet_parking: convert_osm::PrivateOffstreetParking,
    /// OSM railway=rail will be included as light rail if so. Cosmetic only.
    pub include_railroads: bool,
    /// If provided, read polygons from this GeoJSON file and add them to the RawMap as buildings.
    pub extra_buildings: Option<String>,
    pub gtfs: Option<String>,
}

impl GenericCityImporter {
    pub async fn osm_to_raw(
        &self,
        name: MapName,
        timer: &mut abstutil::Timer<'_>,
        config: &ImporterConfiguration,
    ) -> RawMap {
        let local_osm_file = if self.osm_url.starts_with("http") {
            let file = name.city.input_path(format!(
                "osm/{}",
                std::path::Path::new(&self.osm_url)
                    .file_name()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap()
            ));
            download(config, file.clone(), &self.osm_url).await;
            file
        } else {
            self.osm_url.clone()
        };

        osmconvert(
            local_osm_file,
            format!(
                "importer/config/{}/{}/{}.poly",
                name.city.country, name.city.city, name.map
            ),
            name.city.input_path(format!("osm/{}.osm", name.map)),
            config,
        );

        // TODO Download from the GTFS url, stick it in name.city.input_path("gtfs")

        let map = convert_osm::convert(
            convert_osm::Options {
                osm_input: name.city.input_path(format!("osm/{}.osm", name.map)),
                name: name.clone(),

                clip: Some(format!(
                    "importer/config/{}/{}/{}.poly",
                    name.city.country, name.city.city, name.map
                )),
                map_config: self.map_config.clone(),
                onstreet_parking: self.onstreet_parking.clone(),
                public_offstreet_parking: self.public_offstreet_parking.clone(),
                private_offstreet_parking: self.private_offstreet_parking.clone(),
                include_railroads: self.include_railroads,
                extra_buildings: self.extra_buildings.clone(),
                gtfs: self.gtfs.clone(),
            },
            timer,
        );
        map.save();
        map
    }
}

use abstutil::Timer;
use map_model::Map;

use crate::CensusArea;

impl CensusArea {
    pub fn find_data_for_map(map: &Map, timer: &mut Timer) -> Result<Vec<CensusArea>, String> {
        // TODO importer/src/utils.rs has a download() helper that we could copy here. (And later
        // dedupe, after deciding how this crate will integrate with the importer)
        let name = map.get_name();
        let mut shapes = abstutil::read_binary::<ExtraShapes>(kml_path.to_string(), timer);
        abstutil::load_bin(format!("system/{}/{}/popdata.json", name.city, name.map));
    }
}

use map_model::Map;

use crate::CensusArea;

impl CensusArea {
    pub fn find_data_for_map(_map: &Map) -> Result<Vec<CensusArea>, String> {
        // TODO importer/src/utils.rs has a download() helper that we could copy here. (And later
        // dedupe, after deciding how this crate will integrate with the importer)
        todo!()
    }
}

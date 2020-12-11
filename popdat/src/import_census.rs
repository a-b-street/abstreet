use crate::CensusArea;
use abstutil::Timer;
use geojson::GeoJson;
use map_model::Map;
use std::convert::TryInto;

impl CensusArea {
    pub fn find_data_for_map(map: &Map, timer: &mut Timer) -> Result<Vec<CensusArea>, String> {
        // TODO eventually it'd be nice to lazily download the info needed. For now we expect a
        // prepared geojson file to exist in data/system/<city>/population_areas.geojson
        //
        // When we implement downloading, importer/src/utils.rs has a download() helper that we
        // could copy here. (And later dedupe, after deciding how this crate will integrate with
        // the importer)
        let path = abstutil::path(format!(
            "system/{}/population_areas.geojson",
            map.get_name().city
        ));
        let bytes = abstutil::slurp_file(&path)?;
        debug!("parsing geojson at path: {}", &path);

        // TODO - can we change to Result<_,Box<dyn std::error::Error>> and avoid all these map_err?
        let str = String::from_utf8(bytes).map_err(|e| e.to_string())?;
        let geojson = str.parse::<GeoJson>().map_err(|e| e.to_string())?;
        let mut results = vec![];
        if let GeoJson::FeatureCollection(collection) = geojson {
            debug!("collection.features: {}", &collection.features.len());
            for feature in collection.features {
                let properties = feature.properties.expect("malformed population data.");

                let total_population = match properties.get("population").unwrap() {
                    serde_json::Value::Number(n) => n
                        .as_u64()
                        .expect(&format!("unexpected total population number: {:?}", n))
                        as usize,
                    _ => {
                        return Err(format!(
                            "unexpected format for 'population': {:?}",
                            properties.get("population")
                        ));
                    }
                };

                let geometry = feature.geometry.expect("geojson feature missing geometry");
                debug!("geometry: {:?}", &geometry);
                use std::convert::TryFrom;
                let multi_poly: Result<geo::MultiPolygon<f64>, _> = geometry.value.try_into();
                let mut multi_poly = multi_poly.map_err(|e| e.to_string())?;
                let geo_polygon = multi_poly
                    .0
                    .pop()
                    .expect("multipolygon was unexpectedly empty");
                if !multi_poly.0.is_empty() {
                    // Annoyingly upstream GIS has packaged all these individual polygons into
                    // "multipolygon" of length 1 Make sure nothing surprising is
                    // happening since we only use the first poly
                    error!(
                        "feature unexpectedly had multiple polygons: {:?}",
                        &properties
                    );
                }

                let polygon = geom::Polygon::from(geo_polygon);
                results.push(CensusArea {
                    polygon,
                    total_population,
                });
            }
        } else {
            error!("unexpected geojson contents");
        }
        Ok(results)
    }
}

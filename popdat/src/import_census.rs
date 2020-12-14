use crate::CensusArea;
use abstutil::Timer;
use geo::algorithm::intersects::Intersects;
use geojson::GeoJson;
use map_model::Map;
use std::convert::TryFrom;

impl CensusArea {
    pub fn find_data_for_map(map: &Map, timer: &mut Timer) -> anyhow::Result<Vec<CensusArea>> {
        // TODO eventually it'd be nice to lazily download the info needed. For now we expect a
        // prepared geojson file to exist in data/system/<city>/population_areas.geojson
        //
        // expected geojson formatted contents like:
        // {
        //     "type": "FeatureCollection",
        //     "features": [
        //          {
        //              "type": "Feature",
        //              "properties": { "population": 123 },
        //              "geometry": {
        //                  "type": "MultiPolygon",
        //                  "coordinates": [ [ [ [ -73.7, 40.8 ], [ -73.7, 40.8 ], ...] ] ] ]
        //               }
        //          },
        //          {
        //              "type": "Feature",
        //              "properties": { "population": 249 },
        //              "geometry": {
        //                  "type": "MultiPolygon",
        //                  "coordinates": [ [ [ [ -73.8, 40.8 ], [ -73.8, 40.8 ], ...] ] ]
        //               }
        //           },
        //          ...
        //      ]
        // }
        //
        // Note: intuitively you might expect a collection of Polygon's rather than  MultiPolygons,
        // but anecdotally, the census data I've seen uses MultiPolygons. In practice almost
        // all are MultiPoly's with just one element, but some truly have multiple polygons.
        //
        // When we implement downloading, importer/src/utils.rs has a download() helper that we
        // could copy here. (And later dedupe, after deciding how this crate will integrate with
        // the importer)
        let path = abstutil::path(format!(
            "system/{}/population_areas.geojson",
            map.get_name().city
        ));
        let bytes = abstutil::slurp_file(&path).map_err(|s| anyhow!(s))?;
        debug!("parsing geojson at path: {}", &path);
        timer.start("parsing geojson");
        let geojson = GeoJson::from_reader(&*bytes)?;
        timer.stop("parsing geojson");
        let mut results = vec![];
        let collection = geojson::FeatureCollection::try_from(geojson)?;

        let map_area = map.get_boundary_polygon();
        let bounds = map.get_gps_bounds();

        use geo::algorithm::map_coords::MapCoordsInplace;
        let mut geo_map_area: geo::Polygon<_> = map_area.clone().into();
        geo_map_area.map_coords_inplace(|c| {
            let projected = geom::Pt2D::new(c.0, c.1).to_gps(bounds);
            (projected.x(), projected.y())
        });

        debug!("collection.features: {}", &collection.features.len());
        timer.start("converting to `CensusArea`s");
        for feature in collection.features.into_iter() {
            let population = match feature
                .property("population")
                .ok_or(anyhow!("missing 'population' property"))?
            {
                serde_json::Value::Number(n) => n
                    .as_u64()
                    .ok_or(anyhow!("unexpected population number: {:?}", n))?
                    as usize,
                _ => {
                    bail!(
                        "unexpected format for 'population': {:?}",
                        feature.property("population")
                    );
                }
            };

            let geometry = feature
                .geometry
                .ok_or(anyhow!("geojson feature missing geometry"))?;
            let mut multi_poly = geo::MultiPolygon::<f64>::try_from(geometry.value)?;
            let mut geo_polygon = multi_poly
                .0
                .pop()
                .ok_or(anyhow!("multipolygon was unexpectedly empty"))?;
            if !multi_poly.0.is_empty() {
                // I haven't looked into why this is happening - but intuitively a census area could
                // include separate polygons - e.g. across bodies of water. In practice they are a
                // vast minority, so we naively just take the first one for now.
                warn!(
                    "dropping {} polygons from census area with multiple polygons",
                    multi_poly.0.len()
                );
            }

            if !geo_polygon.intersects(&geo_map_area) {
                debug!(
                    "skipping polygon outside of map area. polygon: {:?}, map_area: {:?}",
                    geo_polygon, geo_map_area
                );
                continue;
            }

            geo_polygon.map_coords_inplace(|(x, y)| {
                let point = geom::LonLat::new(*x, *y).to_pt(bounds);
                (point.x(), point.y())
            });
            let polygon = geom::Polygon::from(geo_polygon);
            results.push(CensusArea {
                polygon,
                population,
            });
        }
        debug!("built {} CensusAreas within map bounds", results.len());
        timer.stop("converting to `CensusArea`s");

        Ok(results)
    }
}

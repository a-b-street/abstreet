use anyhow::Result;

use geom::{GPSBounds, Polygon};

use crate::CensusArea;

impl CensusArea {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn fetch_all_for_map(
        map_area: &Polygon,
        bounds: &GPSBounds,
    ) -> Result<Vec<CensusArea>> {
        use flatgeobuf::HttpFgbReader;
        use geo::{BoundingRect, Intersects, MapCoordsInPlace};
        use geozero::geo_types::GeoWriter;

        let mut geo_map_area: geo::Polygon = map_area.clone().into();
        geo_map_area.map_coords_in_place(|c| {
            let projected = geom::Pt2D::new(c.x, c.y).to_gps(bounds);
            (projected.x(), projected.y()).into()
        });

        let bounding_rect = geo_map_area
            .bounding_rect()
            .ok_or_else(|| anyhow!("missing bound rect"))?;

        // See the import handbook for how to prepare this file.
        let mut fgb = HttpFgbReader::open("https://abstreet.s3.amazonaws.com/population_areas.fgb")
            .await?
            .select_bbox(
                bounding_rect.min().x,
                bounding_rect.min().y,
                bounding_rect.max().x,
                bounding_rect.max().y,
            )
            .await?;

        let mut results = vec![];
        while let Some(feature) = fgb.next().await? {
            use flatgeobuf::FeatureProperties;
            // PERF TODO: how to parse into usize directly? And avoid parsing entire props dict?
            let props = feature.properties()?;
            if !props.contains_key("population") {
                warn!("skipping feature with missing population");
                continue;
            }
            let population: usize = props["population"].parse()?;
            let geometry = match feature.geometry() {
                Some(g) => g,
                None => {
                    warn!("skipping feature with missing geometry");
                    continue;
                }
            };
            let mut geo = GeoWriter::new();
            geometry.process(&mut geo, flatgeobuf::GeometryType::MultiPolygon)?;
            if let Some(geo::Geometry::MultiPolygon(multi_poly)) = geo.take_geometry() {
                let geo_polygon = multi_poly
                    .0
                    .first()
                    .ok_or_else(|| anyhow!("multipolygon was unexpectedly empty"))?;
                if multi_poly.0.len() > 1 {
                    warn!(
                        "dropping {} extra polygons from census area: {:?}",
                        multi_poly.0.len() - 1,
                        props
                    );
                }

                if !geo_polygon.intersects(&geo_map_area) {
                    debug!(
                        "skipping polygon outside of map area. polygon: {:?}, map_area: {:?}",
                        geo_polygon, geo_map_area
                    );
                    continue;
                }

                let mut polygon = geo_polygon.clone();
                polygon.map_coords_in_place(|c| geom::LonLat::new(c.x, c.y).to_pt(bounds).into());
                results.push(CensusArea {
                    polygon,
                    population,
                });
            } else {
                warn!("skipping unexpected geometry");
                continue;
            }
        }

        Ok(results)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn fetch_all_for_map(_: &Polygon, _: &GPSBounds) -> Result<Vec<CensusArea>> {
        bail!("Unsupported on web");
    }
}

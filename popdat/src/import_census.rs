use anyhow::Result;
use geo::algorithm::intersects::Intersects;

use geom::{GPSBounds, Polygon};

use crate::CensusArea;

impl CensusArea {
    pub async fn fetch_all_for_map(
        map_area: &Polygon,
        bounds: &GPSBounds,
    ) -> Result<Vec<CensusArea>> {
        use flatgeobuf::HttpFgbReader;
        use geozero_core::geo_types::Geo;

        use geo::algorithm::{bounding_rect::BoundingRect, map_coords::MapCoordsInplace};
        let mut geo_map_area: geo::Polygon<_> = map_area.clone().into();
        geo_map_area.map_coords_inplace(|c| {
            let projected = geom::Pt2D::new(c.0, c.1).to_gps(bounds);
            (projected.x(), projected.y())
        });

        // See the import handbook for how to prepare this file.
        let mut fgb =
            HttpFgbReader::open("https://abstreet.s3.amazonaws.com/population_areas.fgb").await?;

        let bounding_rect = geo_map_area
            .bounding_rect()
            .ok_or(anyhow!("missing bound rect"))?;
        fgb.select_bbox(
            bounding_rect.min().x,
            bounding_rect.min().y,
            bounding_rect.max().x,
            bounding_rect.max().y,
        )
        .await?;

        let mut results = vec![];
        while let Some(feature) = fgb.next().await? {
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
            let mut geo = Geo::new();
            geometry.process(&mut geo, flatgeobuf::GeometryType::MultiPolygon)?;
            if let geo::Geometry::MultiPolygon(multi_poly) = geo.geometry() {
                let geo_polygon = multi_poly
                    .0
                    .first()
                    .ok_or(anyhow!("multipolygon was unexpectedly empty"))?;
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
                polygon.map_coords_inplace(|(x, y)| {
                    let point = geom::LonLat::new(*x, *y).to_pt(bounds);
                    (point.x(), point.y())
                });
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
}

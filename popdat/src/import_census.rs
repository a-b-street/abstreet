use geo::algorithm::intersects::Intersects;

use abstutil::Timer;
use map_model::Map;

use crate::CensusArea;

impl CensusArea {
    pub fn fetch_all_for_map(map: &Map, timer: &mut Timer) -> anyhow::Result<Vec<CensusArea>> {
        timer.start("processing population areas fgb");
        let areas = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime")
            .block_on(Self::fetch_all_for_map_async(map, timer))?;
        timer.stop("processing population areas fgb");
        Ok(areas)
    }

    async fn fetch_all_for_map_async(
        map: &Map,
        timer: &mut Timer<'_>,
    ) -> anyhow::Result<Vec<CensusArea>> {
        use flatgeobuf::HttpFgbReader;
        use geozero_core::geo_types::Geo;

        let map_area = map.get_boundary_polygon();
        let bounds = map.get_gps_bounds();

        use geo::algorithm::{bounding_rect::BoundingRect, map_coords::MapCoordsInplace};
        let mut geo_map_area: geo::Polygon<_> = map_area.clone().into();
        geo_map_area.map_coords_inplace(|c| {
            let projected = geom::Pt2D::new(c.0, c.1).to_gps(bounds);
            (projected.x(), projected.y())
        });

        timer.start("opening FGB reader");
        // See the import handbook for how to prepare this file.
        let mut fgb =
            HttpFgbReader::open("https://abstreet.s3.amazonaws.com/population_areas.fgb").await?;
        timer.stop("opening FGB reader");

        timer.start("selecting bounding box");
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
        timer.stop("selecting bounding box");

        timer.start("processing features");
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
        timer.stop("processing features");

        Ok(results)
    }
}

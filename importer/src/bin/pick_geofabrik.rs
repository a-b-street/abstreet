#[macro_use]
extern crate log;

use std::convert::TryInto;

use anyhow::Result;
use geo::algorithm::area::Area;
use geo::algorithm::contains::Contains;
use geojson::GeoJson;

use abstutil::{CmdArgs, Timer};
use geom::LonLat;

/// Takes an [osmosis polygon boundary
/// file](https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format) as input, then
/// prints the osm.pbf file from download.geofabrik.de that covers this region.
///
/// This is a useful tool when importing a new map, if you don't already know which geofabrik file
/// you should use as your OSM input.
#[tokio::main]
async fn main() -> Result<()> {
    let mut args = CmdArgs::new();
    let input = args.required_free();
    args.done();
    let boundary_pts = LonLat::read_osmosis_polygon(&input)?;
    // For now, just use the boundary's center. Some boundaries might cross multiple geofabrik
    // regions; don't handle that yet.
    let center = LonLat::center(&boundary_pts);

    let geofabrik_idx = load_remote_geojson(
        abstio::path_shared_input("geofabrik-index.json"),
        "https://download.geofabrik.de/index-v1.json",
    )
    .await?;
    let matches = find_matching_regions(geofabrik_idx, center);
    info!(
        "{} regions contain boundary center {}",
        matches.len(),
        center
    );
    // Find the smallest matching region. Just round to the nearest square meter for comparison.
    let (_, url) = matches
        .into_iter()
        .min_by_key(|(mp, _)| mp.unsigned_area() as usize)
        .unwrap();
    println!("{}", url);

    Ok(())
}

async fn load_remote_geojson(path: String, url: &str) -> Result<GeoJson> {
    if !abstio::file_exists(&path) {
        info!("Downloading {}", url);
        abstio::download_to_file(url, None, &path).await?;
    }
    abstio::maybe_read_json(path, &mut Timer::throwaway())
}

fn find_matching_regions(
    geojson: GeoJson,
    center: LonLat,
) -> Vec<(geo::MultiPolygon<f64>, String)> {
    let center: geo::Point<f64> = center.into();

    let mut matches = Vec::new();

    // We're assuming some things about the geofabrik_idx index format -- it's a feature
    // collection, every feature has a multipolygon geometry, the properties have a particular
    // form.
    if let GeoJson::FeatureCollection(fc) = geojson {
        info!("Searching {} regions", fc.features.len());
        for mut feature in fc.features {
            let mp: geo::MultiPolygon<f64> =
                feature.geometry.take().unwrap().value.try_into().unwrap();
            if mp.contains(&center) {
                matches.push((
                    mp,
                    feature
                        .property("urls")
                        .unwrap()
                        .get("pbf")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string(),
                ));
            }
        }
    }

    matches
}

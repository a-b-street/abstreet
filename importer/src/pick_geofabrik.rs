use std::convert::TryInto;

use anyhow::{bail, Result};
use geo::{Area, Contains};
use geojson::GeoJson;

use abstutil::Timer;

/// Given the path to a GeoJSON boundary polygon, return the URL of the smallest Geofabrik osm.pbf
/// file that completely covers the boundary.
pub async fn pick_geofabrik(input: String) -> Result<String> {
    let boundary = load_boundary(input)?;

    let geofabrik_idx = load_remote_geojson(
        abstio::path_shared_input("geofabrik-index.json"),
        "https://download.geofabrik.de/index-v1.json",
    )
    .await?;
    let matches = find_matching_regions(geofabrik_idx, boundary);
    info!("{} regions contain boundary", matches.len(),);
    // Find the smallest matching region. Just round to the nearest square meter for comparison.
    let (_, url) = matches
        .into_iter()
        .min_by_key(|(mp, _)| mp.unsigned_area() as usize)
        .unwrap();
    Ok(url)
}

fn load_boundary(path: String) -> Result<geo::Polygon> {
    let gj: GeoJson = abstio::maybe_read_json(path, &mut Timer::throwaway())?;
    let mut features = match gj {
        GeoJson::Feature(feature) => vec![feature],
        GeoJson::FeatureCollection(feature_collection) => feature_collection.features,
        _ => bail!("Unexpected geojson: {:?}", gj),
    };
    if features.len() != 1 {
        bail!("Expected exactly 1 feature");
    }
    let poly: geo::Polygon = features
        .pop()
        .unwrap()
        .geometry
        .take()
        .unwrap()
        .value
        .try_into()
        .unwrap();
    Ok(poly)
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
    boundary: geo::Polygon,
) -> Vec<(geo::MultiPolygon, String)> {
    let mut matches = Vec::new();

    // We're assuming some things about the geofabrik_idx index format -- it's a feature
    // collection, every feature has a multipolygon geometry, the properties have a particular
    // form.
    if let GeoJson::FeatureCollection(fc) = geojson {
        info!("Searching {} regions", fc.features.len());
        for mut feature in fc.features {
            let mp: geo::MultiPolygon = feature.geometry.take().unwrap().value.try_into().unwrap();
            if mp.contains(&boundary) {
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

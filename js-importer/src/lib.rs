use log::info;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use geom::LonLat;
use map_model::DrivingSide;

// This is a subset of the variation in the cli crate, and geojson_path becomes geojson_boundary
#[derive(Serialize, Deserialize)]
pub struct OneStepImport {
    boundary_polygon: Vec<LonLat>,
    map_name: String,
    driving_side: DrivingSide,
}

#[wasm_bindgen]
pub async fn one_step_import(input: JsValue) -> Result<String, JsValue> {
    // Panics shouldn't happen, but if they do, console.log them.
    console_error_panic_hook::set_once();

    inner(input)
        .await
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

async fn inner(input: JsValue) -> anyhow::Result<String> {
    let input: OneStepImport = input.into_serde()?;
    let osm_xml = download_overpass(&input.boundary_polygon).await?;

    info!("got overpass result {osm_xml}");
    Ok(osm_xml)
}

async fn download_overpass(boundary_polygon: &[LonLat]) -> anyhow::Result<String> {
    let mut filter = "poly:\"".to_string();
    for pt in boundary_polygon {
        filter.push_str(&format!("{} {} ", pt.y(), pt.x()));
    }
    filter.pop();
    filter.push('"');
    // See https://wiki.openstreetmap.org/wiki/Overpass_API/Overpass_QL
    let query = format!(
        "(\n   nwr({});\n     node(w)->.x;\n   <;\n);\nout meta;\n",
        filter
    );
    info!("Querying overpass: {query}");
    abstio::http_post("https://overpass-api.de/api/interpreter", query).await
}

use log::info;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use abstio::MapName;
use abstutil::Timer;
use geom::LonLat;
use map_model::DrivingSide;

// This is a subset of the variation in the cli crate, and geojson_path becomes geojson_boundary
#[derive(Deserialize)]
pub struct OneStepImport {
    boundary_polygon: Vec<LonLat>,
    map_name: String,
    driving_side: DrivingSide,
}

#[wasm_bindgen]
pub async fn one_step_import(input: JsValue) -> Result<JsValue, JsValue> {
    // Panics shouldn't happen, but if they do, console.log them.
    console_error_panic_hook::set_once();
    abstutil::logger::setup();

    inner(input)
        .await
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

async fn inner(input: JsValue) -> anyhow::Result<JsValue> {
    let input: OneStepImport = input.into_serde()?;
    let mut timer = Timer::new("one_step_import");
    timer.start("download from Overpass");
    let osm_xml = download_overpass(&input.boundary_polygon).await?;
    timer.stop("download from Overpass");

    let raw = convert_osm::convert_bytes(
        osm_xml,
        MapName::new("zz", "oneshot", &input.map_name),
        Some(input.boundary_polygon),
        convert_osm::Options::default_for_side(input.driving_side),
        &mut timer,
    );
    let map =
        map_model::Map::create_from_raw(raw, map_model::RawToMapOptions::default(), &mut timer);

    let result = JsValue::from_serde(&map)?;
    Ok(result)
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

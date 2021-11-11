use std::path::Path;

use anyhow::Result;

use abstio::CityName;
use geom::LonLat;

pub async fn run(
    geojson_path: String,
    name: String,
    drive_on_left: bool,
    use_geofabrik: bool,
    filter_crosswalks: bool,
) -> Result<()> {
    if name.contains(' ') || name.is_empty() {
        panic!(
            "--map_name must be non-empty and contain no spaces: {}",
            name
        );
    }

    // Convert to a boundary polygon.
    {
        println!("Converting GeoJSON to Osmosis boundary");
        crate::geojson_to_osmosis::run(geojson_path.clone())?;
        if Path::new("boundary1.poly").exists() {
            abstio::delete_file("boundary0.poly");
            abstio::delete_file("boundary1.poly");
            // If there were more, leave them around, but at least delete these 2, so the user
            // can try again.
            anyhow::bail!(
                "Your GeoJSON contained multiple polygons. You can only import one at a time."
            );
        }
    }

    let city = CityName::new("zz", "oneshot");
    let osm;
    if !use_geofabrik {
        println!("Downloading OSM data from Overpass...");
        osm = city.input_path(format!("osm/{}.osm", name));

        let geojson = abstio::slurp_file(geojson_path)?;
        let mut polygons = LonLat::parse_geojson_polygons(String::from_utf8(geojson)?)?;
        let mut filter = "poly:\"".to_string();
        for pt in polygons.pop().unwrap() {
            filter.push_str(&format!("{} {} ", pt.y(), pt.x()));
        }
        filter.pop();
        filter.push('"');
        // See https://wiki.openstreetmap.org/wiki/Overpass_API/Overpass_QL
        let query = format!(
            "(\n   nwr({});\n     node(w)->.x;\n   <;\n);\nout meta;\n",
            filter
        );
        abstio::download_to_file("https://overpass-api.de/api/interpreter", Some(query), &osm)
            .await?;
    } else {
        println!("Figuring out what Geofabrik file contains your boundary");
        let url = crate::pick_geofabrik::run("boundary0.poly".to_string()).await?;

        let pbf = city.input_path(format!("osm/{}.pbf", abstutil::basename(&url)));
        osm = city.input_path(format!("osm/{}.osm", name));
        std::fs::create_dir_all(std::path::Path::new(&osm).parent().unwrap())
            .expect("Creating parent dir failed");

        // Download it!
        // TODO This is timing out. Also, really could use progress bars.
        if !abstio::file_exists(&pbf) {
            println!("Downloading {}", url);
            abstio::download_to_file(url, None, &pbf).await?;
        }

        // Clip it
        println!("Clipping osm.pbf file to your boundary");
        crate::clip_osm::run(pbf, "boundary0.poly".to_string(), osm.clone())?;
    }

    // Import!
    println!("Running importer");
    importer::oneshot(
        osm,
        Some("boundary0.poly".to_string()),
        !drive_on_left,
        filter_crosswalks,
        map_model::RawToMapOptions::default(),
    );

    // Clean up temporary files. If we broke before this, deliberately leave them around for
    // debugging.
    abstio::delete_file("boundary0.poly");

    Ok(())
}

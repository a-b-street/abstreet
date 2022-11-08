use anyhow::Result;

use abstio::CityName;
use geom::LonLat;

pub async fn run(
    geojson_path: String,
    name: String,
    use_geofabrik: bool,
    filter_crosswalks: bool,
    create_uk_travel_demand_model: bool,
) -> Result<()> {
    if name.contains(' ') || name.is_empty() {
        panic!(
            "--map_name must be non-empty and contain no spaces: {}",
            name
        );
    }

    let city = CityName::new("zz", "oneshot");
    let osm;
    if !use_geofabrik {
        println!("Downloading OSM data from Overpass...");
        osm = city.input_path(format!("osm/{}.osm", name));

        let geojson = abstio::slurp_file(geojson_path.clone())?;
        let mut polygons = LonLat::parse_geojson_polygons(String::from_utf8(geojson)?)?;
        let mut filter = "poly:\"".to_string();
        for pt in polygons.pop().unwrap().0 {
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
        let url = importer::pick_geofabrik(geojson_path.clone()).await?;

        let pbf = city.input_path(format!("osm/{}.pbf", abstutil::basename(&url)));
        osm = city.input_path(format!("osm/{}.osm", name));
        fs_err::create_dir_all(std::path::Path::new(&osm).parent().unwrap())
            .expect("Creating parent dir failed");

        // Download it!
        // TODO This is timing out. Also, really could use progress bars.
        if !abstio::file_exists(&pbf) {
            println!("Downloading {}", url);
            abstio::download_to_file(url, None, &pbf).await?;
        }

        // Clip it
        println!("Clipping osm.pbf file to your boundary");
        crate::clip_osm::run(pbf, geojson_path.clone(), osm.clone())?;
    }

    // Import!
    println!("Running importer");
    importer::oneshot(
        osm,
        Some(geojson_path),
        filter_crosswalks,
        create_uk_travel_demand_model,
        map_model::RawToMapOptions::default(),
    )
    .await;

    Ok(())
}

use geom::{GPSBounds, LonLat};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

pub fn import_parcels(path: &str) -> Result<HashMap<String, LonLat>, failure::Error> {
    let mut coords = BufWriter::new(File::create("/tmp/parcels")?);
    let mut parcel_ids = Vec::new();
    // TODO Timer
    for rec in csv::ReaderBuilder::new()
        .delimiter(b' ')
        .from_reader(File::open(path)?)
        .records()
    {
        let rec = rec?;
        parcel_ids.push(rec[15].to_string());
        coords.write_fmt(format_args!("{} {}\n", &rec[25], &rec[26]))?;
        // TODO convert it all
        if parcel_ids.len() == 100 {
            break;
        }
    }
    coords.flush()?;

    // TODO Ideally we could just do the conversion directly without any dependencies, but the
    // formats are documented quite confusingly. Couldn't get the Rust crate for proj or GDAL
    // bindings to build. So just do this hack.
    let output = std::process::Command::new("cs2cs")
        // cs2cs +init=esri:102748 +to +init=epsg:4326 -f '%.5f' foo
        .args(vec![
            "+init=esri:102748",
            "+to",
            "+init=epsg:4326",
            "-f",
            "%.5f",
            "/tmp/parcels",
        ])
        .output()?;
    assert!(output.status.success());

    let bounds = GPSBounds::seattle_bounds();
    let reader = BufReader::new(output.stdout.as_slice());
    let mut result = HashMap::new();
    for (line, id) in reader.lines().zip(parcel_ids.into_iter()) {
        let line = line?;
        let pieces: Vec<&str> = line.split_whitespace().collect();
        let lon: f64 = pieces[0].parse()?;
        let lat: f64 = pieces[1].parse()?;
        let pt = LonLat::new(lon, lat);
        if bounds.contains(pt) {
            println!("parcel {} is at {}", id, pt);
            result.insert(id, pt);
        }
    }
    Ok(result)
}

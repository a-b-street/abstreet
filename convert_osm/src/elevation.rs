use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::Command;

use anyhow::Result;

use geom::{Distance, PolyLine};
use map_model::raw::{OriginalRoad, RawMap};

pub fn add_data(map: &mut RawMap) -> Result<()> {
    // TODO It'd be nice to include more timing breakdown here, but if we bail out early,
    // it's tedious to call timer.stop().
    let ids = generate_input(map)?;

    std::fs::create_dir_all("elevation_output")?;
    std::fs::create_dir_all(abstio::path_shared_input("elevation"))?;
    let pwd = std::env::current_dir()?.display().to_string();
    // Because elevation_lookups has so many dependencies, just depend on Docker.
    // TODO This is only going to run on Linux, unless we can also build images for other OSes.
    // TODO On Linux, data/input/shared/elevation files wind up being owned by root, due to how
    // docker runs. For the moment, one workaround is to manually fix the owner afterwards:
    // find data/ -user root -exec sudo chown $USER:$USER '{}' \;
    let status = Command::new("docker")
        .arg("run")
        // Bind the input directory to the temporary place we just created
        .arg("--mount")
        .arg(format!(
            "type=bind,source={}/elevation_input,target=/elevation/input,readonly",
            pwd
        ))
        // We want to cache the elevation data sources in A/B Street's S3 bucket, so bind to
        // our data/input/shared directory.
        .arg("--mount")
        .arg(format!(
            "type=bind,source={},target=/elevation/data",
            // Docker requires absolute paths
            format!("{}/{}", pwd, abstio::path_shared_input("elevation"))
        ))
        .arg("--mount")
        .arg(format!(
            "type=bind,source={}/elevation_output,target=/elevation/output",
            pwd
        ))
        .arg("-t")
        // https://hub.docker.com/r/abstreet/elevation_lookups
        .arg("abstreet/elevation_lookups")
        .arg("python3")
        .arg("main.py")
        .arg("query")
        // TODO How to tune this? Pretty machine dependant, and using ALL available cores may
        // melt memory.
        .arg("--n_threads=1")
        .status()?;
    if !status.success() {
        bail!("Command failed: {}", status);
    }

    scrape_output(map, ids)?;

    // Clean up temporary files
    std::fs::remove_file("elevation_input/query")?;
    std::fs::remove_dir("elevation_input")?;
    std::fs::remove_file("elevation_output/query")?;
    std::fs::remove_dir("elevation_output")?;

    Ok(())
}

fn generate_input(map: &RawMap) -> Result<Vec<OriginalRoad>> {
    std::fs::create_dir_all("elevation_input")?;
    let mut f = BufWriter::new(File::create("elevation_input/query")?);
    let mut ids = Vec::new();
    for (id, r) in &map.roads {
        // TODO Handle cul-de-sacs
        if let Ok(pl) = PolyLine::new(r.center_points.clone()) {
            ids.push(*id);
            // Sample points along the road. Smaller step size gives more detail, but is slower.
            let mut pts = Vec::new();
            for (pt, _) in pl.step_along(Distance::meters(5.0), Distance::ZERO) {
                pts.push(pt);
            }
            // Always ask for the intersection
            if *pts.last().unwrap() != pl.last_pt() {
                pts.push(pl.last_pt());
            }
            for (idx, gps) in map.gps_bounds.convert_back(&pts).into_iter().enumerate() {
                write!(f, "{},{}", gps.x(), gps.y())?;
                if idx != pts.len() - 1 {
                    write!(f, " ")?;
                }
            }
            writeln!(f)?;
        }
    }
    Ok(ids)
}

fn scrape_output(map: &mut RawMap, ids: Vec<OriginalRoad>) -> Result<()> {
    let num_ids = ids.len();
    let mut cnt = 0;
    for (line, id) in BufReader::new(File::open("elevation_output/query")?)
        .lines()
        .zip(ids)
    {
        cnt += 1;
        let line = line?;
        let mut values = Vec::new();
        for x in line.split('\t') {
            if let Ok(x) = x.parse::<f64>() {
                if !x.is_finite() {
                    // TODO Warn
                    continue;
                }
                if x < 0.0 {
                    // TODO Temporary
                    continue;
                }
                values.push(Distance::meters(x));
            } else {
                // Blank lines mean the tool failed to figure out what happened
                continue;
            }
        }
        if values.len() != 4 {
            error!("Elevation output line \"{}\" doesn't have 4 numbers", line);
            continue;
        }
        // TODO Also put total_climb and total_descent on the roads
        map.intersections.get_mut(&id.i1).unwrap().elevation = values[0];
        map.intersections.get_mut(&id.i2).unwrap().elevation = values[1];
    }
    if cnt != num_ids {
        bail!("Output had {} lines, but we made {} queries", cnt, num_ids);
    }

    // Calculate the incline for each road here, before the road gets trimmed for intersection
    // geometry. If we did this after trimming, we'd miss some of the horizontal distance.
    for (id, road) in &mut map.roads {
        let rise = map.intersections[&id.i2].elevation - map.intersections[&id.i1].elevation;
        let run = PolyLine::unchecked_new(road.center_points.clone()).length();
        if !(rise / run).is_finite() {
            // TODO Warn?
            continue;
        }
        road.percent_incline = rise / run;
        // Per https://wiki.openstreetmap.org/wiki/Key:incline#Common_.26_extreme_inclines, we
        // shouldn't often see values outside a certain range. Adjust this when we import
        // somewhere exceeding this...
        if road.percent_incline.abs() > 0.3 {
            error!(
                "{} is unexpectedly steep! Incline is {}%",
                id,
                road.percent_incline * 100.0
            );
        }
    }

    Ok(())
}

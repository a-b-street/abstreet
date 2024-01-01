use std::collections::HashMap;
use std::io::BufReader;

use anyhow::Result;
use elevation::GeoTiffElevation;
use fs_err::File;
use geom::Distance;

use abstutil::Timer;
use raw_map::RawMap;

pub fn add_data(map: &mut RawMap, path: &str, timer: &mut Timer) -> Result<()> {
    // Get intersection points from road endpoints, to reduce the number of elevation lookups
    let mut intersection_points = HashMap::new();
    for r in map.streets.roads.values() {
        for (i, pt) in [
            (r.src_i, r.reference_line.first_pt()),
            (r.dst_i, r.reference_line.last_pt()),
        ] {
            intersection_points.insert(i, pt.to_gps(&map.streets.gps_bounds));
        }
    }

    // TODO Download the file if needed?
    let mut elevation = GeoTiffElevation::new(BufReader::new(File::open(path)?));

    timer.start_iter("lookup elevation", intersection_points.len());
    for (i, gps) in intersection_points {
        timer.next();
        if let Some(height) = elevation.get_height_for_lon_lat(gps.x() as f32, gps.y() as f32) {
            if height < 0.0 {
                continue;
            }
            map.elevation_per_intersection
                .insert(i, Distance::meters(height.into()));
        }
    }

    // Calculate the incline for each road here, before the road gets trimmed for intersection
    // geometry. If we did this after trimming, we'd miss some of the horizontal distance.
    for road in map.streets.roads.values() {
        let rise = map.elevation_per_intersection[&road.dst_i]
            - map.elevation_per_intersection[&road.src_i];
        let run = road.untrimmed_length();
        if !(rise / run).is_finite() {
            // TODO Warn?
            continue;
        }
        let data = map.extra_road_data.get_mut(&road.id).unwrap();
        data.percent_incline = rise / run;
        // Per https://wiki.openstreetmap.org/wiki/Key:incline#Common_.26_extreme_inclines, we
        // shouldn't often see values outside a certain range. Adjust this when we import
        // somewhere exceeding this...
        if data.percent_incline.abs() > 0.3 {
            error!(
                "{} is unexpectedly steep! Incline is {}%",
                road.id,
                data.percent_incline * 100.0
            );
        }
    }

    Ok(())
}

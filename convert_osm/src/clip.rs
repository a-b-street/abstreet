use abstutil::{retain_btreemap, Timer};
use geom::{GPSBounds, LonLat, PolyLine, Polygon};
use map_model::{raw_data, IntersectionType};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn clip_map(map: &mut raw_data::Map, path: &str, timer: &mut Timer) -> GPSBounds {
    timer.start("clipping map to boundary");
    map.boundary_polygon = read_osmosis_polygon(path);
    let bounds = map.get_gps_bounds();

    if true {
        timer.stop("clipping map to boundary");
        return bounds;
    }

    let boundary_poly = Polygon::new(&bounds.must_convert(&map.boundary_polygon));
    let boundary_lines: Vec<PolyLine> = boundary_poly
        .points()
        .windows(2)
        .map(|pair| PolyLine::new(pair.to_vec()))
        .collect();

    // This is kind of indirect and slow, but first pass -- just remove roads completely outside
    // the boundary polygon.
    retain_btreemap(&mut map.roads, |_, r| {
        let center_pts = bounds.must_convert(&r.points);
        let first_in = boundary_poly.contains_pt(center_pts[0]);
        let last_in = boundary_poly.contains_pt(*center_pts.last().unwrap());
        first_in || last_in
    });

    let road_ids: Vec<raw_data::StableRoadID> = map.roads.keys().cloned().collect();
    for id in road_ids {
        let r = &map.roads[&id];
        let center_pts = bounds.must_convert(&r.points);
        let first_in = boundary_poly.contains_pt(center_pts[0]);
        let last_in = boundary_poly.contains_pt(*center_pts.last().unwrap());

        if first_in && last_in {
            continue;
        }

        let move_i = if first_in { r.i2 } else { r.i1 };

        // The road crosses the boundary. But if the intersection happens to have another connected
        // road, then just allow this exception.
        // TODO But what about a road slightly outside the bounds that'd otherwise connect two
        // things in bounds? Really ought to flood outwards and see if we wind up back inside.
        if map
            .roads
            .values()
            .filter(|r2| r2.i1 == move_i || r2.i1 == move_i)
            .count()
            > 1
        {
            println!(
                "{} crosses boundary, but briefly enough to not touch it",
                id
            );
            continue;
        }

        let i = map.intersections.get_mut(&move_i).unwrap();
        i.intersection_type = IntersectionType::Border;

        // Convert the road points to a PolyLine here. Loop roads were breaking!
        let center = PolyLine::new(center_pts);

        // Now trim it.
        let mut_r = map.roads.get_mut(&id).unwrap();
        let border_pt = boundary_lines
            .iter()
            .find_map(|l| center.intersection(l).map(|(pt, _)| pt))
            .unwrap();
        if first_in {
            mut_r.points =
                bounds.must_convert_back(center.get_slice_ending_at(border_pt).unwrap().points());
            i.point = *mut_r.points.last().unwrap();
        } else {
            mut_r.points = bounds.must_convert_back(
                center
                    .reversed()
                    .get_slice_ending_at(border_pt)
                    .unwrap()
                    .reversed()
                    .points(),
            );
            i.point = mut_r.points[0];
        }
    }

    timer.stop("clipping map to boundary");
    bounds
}

fn read_osmosis_polygon(path: &str) -> Vec<LonLat> {
    let mut pts: Vec<LonLat> = Vec::new();
    for (idx, maybe_line) in BufReader::new(File::open(path).unwrap())
        .lines()
        .enumerate()
    {
        if idx == 0 || idx == 1 {
            continue;
        }
        let line = maybe_line.unwrap();
        if line == "END" {
            break;
        }
        let parts: Vec<&str> = line.trim_start().split("    ").collect();
        assert!(parts.len() == 2);
        let lon = parts[0].parse::<f64>().unwrap();
        let lat = parts[1].parse::<f64>().unwrap();
        pts.push(LonLat::new(lon, lat));
    }
    pts
}

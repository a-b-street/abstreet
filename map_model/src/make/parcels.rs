use dimensioned::si;
use geom::{Bounds, HashablePt2D, Line, Pt2D};
use make::sidewalk_finder::find_sidewalk_points;
use raw_data;
use std::collections::HashSet;
use {Lane, Parcel, ParcelID};

pub(crate) fn make_all_parcels(
    results: &mut Vec<Parcel>,
    input: &Vec<raw_data::Parcel>,
    bounds: &Bounds,
    lanes: &Vec<Lane>,
) {
    let mut pts_per_parcel: Vec<Vec<Pt2D>> = Vec::new();
    let mut center_per_parcel: Vec<HashablePt2D> = Vec::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    for p in input {
        let pts = p
            .points
            .iter()
            .map(|coord| Pt2D::from_gps(coord, bounds))
            .collect();
        let center: HashablePt2D = Pt2D::center(&pts).into();
        pts_per_parcel.push(pts);
        center_per_parcel.push(center);
        query.insert(center);
    }

    let sidewalk_pts = find_sidewalk_points(query, lanes);

    for (idx, center) in center_per_parcel.into_iter().enumerate() {
        let (sidewalk, dist_along) = sidewalk_pts[&center];
        let (sidewalk_pt, _) = lanes[sidewalk.0].dist_along(dist_along);
        let line = Line::new(center.into(), sidewalk_pt);
        // Trim parcels that are too far away from the nearest sidewalk
        if line.length() > 100.0 * si::M {
            continue;
        }

        let id = ParcelID(results.len());
        results.push(Parcel {
            id,
            points: pts_per_parcel[idx].clone(),
            block: input[idx].block,
        });
    }
    let discarded = input.len() - results.len();
    if discarded > 0 {
        println!(
            "Discarded {} parcels that weren't close enough to a sidewalk",
            discarded
        );
    }
}

use dimensioned::si;
use geom::{Bounds, HashablePt2D, Pt2D};
use make::sidewalk_finder::find_sidewalk_points;
use raw_data;
use std::collections::HashSet;
use {Lane, Parcel, ParcelID};

pub fn make_all_parcels(
    results: &mut Vec<Parcel>,
    input: &Vec<raw_data::Parcel>,
    gps_bounds: &Bounds,
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
            .map(|coord| Pt2D::from_gps(*coord, gps_bounds).unwrap())
            .collect();
        let center: HashablePt2D = Pt2D::center(&pts).into();
        pts_per_parcel.push(pts);
        center_per_parcel.push(center);
        query.insert(center);
    }

    // Trim parcels that are too far away from the nearest sidewalk
    let sidewalk_pts = find_sidewalk_points(bounds, query, lanes, 100.0 * si::M);

    for (idx, center) in center_per_parcel.into_iter().enumerate() {
        if sidewalk_pts.contains_key(&center) {
            let id = ParcelID(results.len());
            results.push(Parcel {
                id,
                points: pts_per_parcel[idx].clone(),
                block: input[idx].block,
            });
        }
    }
    let discarded = input.len() - results.len();
    if discarded > 0 {
        info!(
            "Discarded {} parcels that weren't close enough to a sidewalk",
            discarded
        );
    }
}

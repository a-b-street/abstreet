use geo;
use geo::prelude::Intersects;
use graphics::math::Vec2d;
use map_model::{BuildingID, IntersectionID, ParcelID, RoadID};
use render;

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
enum ID {
    Road(RoadID),
    Intersection(IntersectionID),
    Building(BuildingID),
    Parcel(ParcelID),
}

// Eventually this should be part of an interactive map fixing pipeline. Find problems, jump to
// them, ask for the resolution, record it.
pub fn validate_geometry(draw_map: &render::DrawMap) {
    let mut objects: Vec<(ID, geo::Polygon<f64>)> = Vec::new();
    for r in &draw_map.roads {
        for poly in &r.polygons {
            objects.push((ID::Road(r.id), make_poly(poly)));
        }
    }
    for i in &draw_map.intersections {
        objects.push((ID::Intersection(i.id), make_poly(&i.polygon)));
    }
    for b in &draw_map.buildings {
        objects.push((ID::Building(b.id), make_poly(&b.polygon)));
    }
    for p in &draw_map.parcels {
        objects.push((ID::Parcel(p.id), make_poly(&p.fill_polygon)));
    }

    println!(
        "{} objects total. About {} possible overlaps",
        objects.len(),
        objects.len().pow(2)
    );

    // TODO use a quadtree to prune
    for (id1, poly1) in &objects {
        for (id2, poly2) in &objects {
            // Overlaps are symmetric and we're not worried about self-intersection, so only
            // check when id1 < id2.
            if id1 >= id2 {
                continue;
            }
            // Buildings and parcels are expected to overlap.
            match (id1, id2) {
                (ID::Building(_), ID::Parcel(_)) => continue,
                (ID::Parcel(_), ID::Building(_)) => continue,
                _ => {}
            };
            if poly1.intersects(poly2) {
                println!("{:?} and {:?} overlap", id1, id2);
            }
        }
    }
}

fn make_poly(points: &Vec<Vec2d>) -> geo::Polygon<f64> {
    let exterior: Vec<geo::Point<f64>> = points
        .iter()
        .map(|pt| geo::Point::new(pt[0], pt[1]))
        .collect();
    geo::Polygon::new(exterior.into(), Vec::new())
}

use generator;
use geo;
use ezgui::input::UserInput;
use piston::input::Key;
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
pub struct Validator {
    gen: generator::Generator<'static, (), (ID, ID)>,
    current_problem: Option<(ID, ID)>,
}

impl Validator {
    pub fn new(draw_map: &render::DrawMap) -> Validator {
        let mut objects: Vec<(ID, Vec<geo::Polygon<f64>>)> = Vec::new();
        for r in &draw_map.roads {
            objects.push((ID::Road(r.id), r.polygons.iter().map(|poly| make_poly(poly)).collect()));
        }
        for i in &draw_map.intersections {
            objects.push((ID::Intersection(i.id), vec![make_poly(&i.polygon)]));
        }
        for b in &draw_map.buildings {
            objects.push((ID::Building(b.id), vec![make_poly(&b.polygon)]));
        }
        for p in &draw_map.parcels {
            objects.push((ID::Parcel(p.id), vec![make_poly(&p.fill_polygon)]));
        }

        println!(
            "{} objects total. About {} possible overlaps",
            objects.len(),
            objects.len().pow(2)
        );

        // TODO scoped vs unscoped?
        let gen = generator::Gn::<()>::new_scoped(move |mut s| {
            // TODO use a quadtree to prune
            for (id1, ls1) in &objects {
                for (id2, ls2) in &objects {
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

                    'outer: for poly1 in ls1 {
                        for poly2 in ls2 {
                            if poly1.intersects(poly2) {
                                s.yield_((*id1, *id2));
                                break 'outer;
                            }
                        }
                    }
                }
            }
            done!();
        });

        Validator {
            gen,
            current_problem: None,
        }
    }

    pub fn event(&mut self, input: &mut UserInput) -> bool {
        // Initialize or advance?
        if !self.current_problem.is_some() || input.key_pressed(Key::N, "Press N to see the next problem") {
            self.current_problem = self.gen.next();

            if let Some((id1, id2)) = self.current_problem {
                println!("{:?} and {:?} intersect", id1, id2);
                return false;
            } else {
                println!("No more problems!");
                return true;
            }
        }

        if input.key_pressed(Key::Escape, "Press Escape to stop looking at problems") {
            println!("Quit geometry validator");
            return true;
        }

        // Later, keys for resolving problems
        false
    }
}

fn make_poly(points: &Vec<Vec2d>) -> geo::Polygon<f64> {
    let exterior: Vec<geo::Point<f64>> = points
        .iter()
        .map(|pt| geo::Point::new(pt[0], pt[1]))
        .collect();
    geo::Polygon::new(exterior.into(), Vec::new())
}

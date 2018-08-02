use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use generator;
use geo;
use geo::prelude::Intersects;
use geom::Pt2D;
use graphics::math::Vec2d;
use map_model::{geometry, BuildingID, IntersectionID, LaneID, Map, ParcelID};
use piston::input::Key;
use render;

// TODO just have one of these
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum ID {
    Lane(LaneID),
    Intersection(IntersectionID),
    Building(BuildingID),
    Parcel(ParcelID),
}

// Eventually this should be part of an interactive map fixing pipeline. Find problems, jump to
// them, ask for the resolution, record it.
pub enum Validator {
    Inactive,
    Active {
        gen: generator::Generator<'static, (), (ID, ID)>,
        current_problem: Option<(ID, ID)>,
    },
}

impl Validator {
    pub fn new() -> Validator {
        Validator::Inactive
    }

    pub fn start(draw_map: &render::DrawMap) -> Validator {
        let mut objects: Vec<(ID, Vec<geo::Polygon<f64>>)> = Vec::new();
        for l in &draw_map.lanes {
            objects.push((
                ID::Lane(l.id),
                l.polygons.iter().map(|poly| make_poly(poly)).collect(),
            ));
        }
        for i in &draw_map.intersections {
            objects.push((ID::Intersection(i.id), vec![make_poly(&i.polygon)]));
        }
        for b in &draw_map.buildings {
            objects.push((ID::Building(b.id), vec![make_poly(&b.fill_polygon)]));
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

        Validator::Active {
            gen,
            current_problem: None,
        }
    }

    pub fn event(&mut self, input: &mut UserInput, canvas: &mut Canvas, map: &Map) -> bool {
        let mut new_state: Option<Validator> = None;
        let active = match self {
            Validator::Inactive => false,
            Validator::Active {
                gen,
                current_problem,
            } => {
                // Initialize or advance?
                if !current_problem.is_some() || input.key_pressed(Key::N, "see the next problem") {
                    // TODO do this in a bg thread or something
                    *current_problem = gen.next();

                    if let Some((id1, id2)) = current_problem {
                        println!("{:?} and {:?} intersect", id1, id2);
                        let pt = get_pt(map, *id1);
                        canvas.center_on_map_pt(pt.x(), pt.y());
                    // TODO also modify selection state to highlight stuff?
                    } else {
                        println!("No more problems!");
                        new_state = Some(Validator::Inactive);
                    }
                } else if input.key_pressed(Key::Escape, "stop looking at problems") {
                    println!("Quit geometry validator");
                    new_state = Some(Validator::Inactive);
                }

                // Later, keys for resolving problems
                true
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        active
    }
}

fn make_poly(points: &Vec<Vec2d>) -> geo::Polygon<f64> {
    let exterior: Vec<geo::Point<f64>> = points
        .iter()
        .map(|pt| geo::Point::new(pt[0], pt[1]))
        .collect();
    geo::Polygon::new(exterior.into(), Vec::new())
}

// TODO duplicated with warp. generic handling of object types?
fn get_pt(map: &Map, id: ID) -> Pt2D {
    match id {
        ID::Lane(id) => map.get_l(id).first_pt(),
        ID::Intersection(id) => map.get_i(id).point,
        ID::Building(id) => geometry::center(&map.get_b(id).points),
        ID::Parcel(id) => geometry::center(&map.get_p(id).points),
    }
}

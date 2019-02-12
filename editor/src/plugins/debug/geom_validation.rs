use crate::objects::ID;
use crate::plugins::{BlockingPlugin, PluginCtx};
use crate::render::DrawMap;
use generator;
use generator::done;
use geo;
use geo::prelude::Intersects;
use geom::Polygon;
use map_model::{LaneType, Map, RoadID};
use std::collections::BTreeSet;

// Eventually this should be part of an interactive map fixing pipeline. Find problems, jump to
// them, ask for the resolution, record it.
pub struct Validator {
    gen: generator::Generator<'static, (), (ID, ID)>,
    current_problem: Option<(ID, ID)>,
}

impl Validator {
    pub fn new(ctx: &mut PluginCtx) -> Option<Validator> {
        if ctx.input.action_chosen("validate map geometry") {
            // TODO Kinda temporarily stuck here for convenience
            list_problems(&ctx.primary.map);
            return Some(Validator::start(&ctx.primary.map, &ctx.primary.draw_map));
        }
        None
    }

    fn start(map: &Map, draw_map: &DrawMap) -> Validator {
        let mut objects: Vec<(ID, Vec<geo::Polygon<f64>>)> = Vec::new();
        for l in &draw_map.lanes {
            objects.push((ID::Lane(l.id), make_polys(&l.polygon)));
        }
        for i in &draw_map.intersections {
            objects.push((ID::Intersection(i.id), make_polys(&i.polygon)));
        }
        for b in map.all_buildings() {
            objects.push((ID::Building(b.id), make_polys(&b.polygon)));
        }
        for p in &draw_map.parcels {
            objects.push((ID::Parcel(p.id), make_polys(&p.fill_polygon)));
        }

        info!(
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
}

impl BlockingPlugin for Validator {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Geometry Debugger", &ctx.canvas);

        // Initialize or advance?
        if self.current_problem.is_none() || ctx.input.modal_action("see next problem") {
            // TODO do this in a bg thread or something
            self.current_problem = self.gen.next();

            if let Some((id1, id2)) = self.current_problem {
                info!("{:?} and {:?} intersect", id1, id2);
                ctx.canvas.center_on_map_pt(
                    id1.canonical_point(&ctx.primary.map, &ctx.primary.sim, &ctx.primary.draw_map)
                        .unwrap(),
                );
            // TODO also modify selection state to highlight stuff?
            } else {
                info!("No more problems!");
                return false;
            }
        } else if ctx.input.modal_action("quit") {
            return false;
        }

        true
    }
}

fn make_polys(p: &Polygon) -> Vec<geo::Polygon<f64>> {
    let mut result = Vec::new();
    for tri in p.triangles() {
        let exterior = vec![
            geo::Point::new(tri.pt1.x(), tri.pt1.y()),
            geo::Point::new(tri.pt2.x(), tri.pt2.y()),
            geo::Point::new(tri.pt3.x(), tri.pt3.y()),
        ];
        result.push(geo::Polygon::new(exterior.into(), Vec::new()));
    }
    result
}

fn list_problems(map: &Map) {
    for i in map.all_intersections() {
        if !i.is_degenerate() {
            continue;
        }
        let road_ids: Vec<RoadID> = i.roads.iter().cloned().collect();
        let r1 = map.get_r(road_ids[0]);
        let r2 = map.get_r(road_ids[1]);

        if r1
            .incoming_lanes(i.id)
            .iter()
            .map(|(_, lt)| *lt)
            .collect::<Vec<LaneType>>()
            != r2
                .outgoing_lanes(i.id)
                .iter()
                .map(|(_, lt)| *lt)
                .collect::<Vec<LaneType>>()
        {
            continue;
        }
        if r1
            .outgoing_lanes(i.id)
            .iter()
            .map(|(_, lt)| *lt)
            .collect::<Vec<LaneType>>()
            != r2
                .incoming_lanes(i.id)
                .iter()
                .map(|(_, lt)| *lt)
                .collect::<Vec<LaneType>>()
        {
            continue;
        }

        error!(
            "{} may be collapsible. OSM ways {} vs {}",
            i.id, r1.osm_way_id, r2.osm_way_id
        );

        let tags1: BTreeSet<(&String, &String)> = r1.osm_tags.iter().collect();
        let tags2: BTreeSet<(&String, &String)> = r2.osm_tags.iter().collect();
        let common = tags1.intersection(&tags2).cloned().collect();
        for (k, v) in tags1.difference(&common) {
            if k.starts_with("tiger:")
                || k.starts_with("old_name:")
                || k.starts_with("destination")
                || k == &"source"
            {
                continue;
            }
            error!("  only {} has: {} = {}", r1.id, k, v);
        }
        for (k, v) in tags2.difference(&common) {
            if k.starts_with("tiger:")
                || k.starts_with("old_name:")
                || k.starts_with("destination")
                || k == &"source"
            {
                continue;
            }
            error!("  only {} has: {} = {}", r2.id, k, v);
        }
    }
}

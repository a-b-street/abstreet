use crate::common::Warping;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{Autocomplete, EventCtx, EventLoopMode, GfxCtx, InputResult, Warper};
use map_model::RoadID;
use std::collections::HashSet;

pub struct Navigator {
    autocomplete: Autocomplete<RoadID>,
}

impl Navigator {
    pub fn new(ui: &UI) -> Navigator {
        // TODO Canonicalize names, handling abbreviations like east/e and street/st
        Navigator {
            autocomplete: Autocomplete::new(
                "Warp where?",
                ui.primary
                    .map
                    .all_roads()
                    .iter()
                    .map(|r| (r.get_name(), r.id))
                    .collect(),
            ),
        }
    }
}

impl State for Navigator {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let map = &ui.primary.map;
        match self.autocomplete.event(ctx.input) {
            InputResult::Canceled => Transition::Pop,
            InputResult::Done(name, ids) => {
                // Roads share intersections, so of course there'll be overlap here.
                let mut cross_streets = HashSet::new();
                for r in &ids {
                    let road = map.get_r(*r);
                    for i in &[road.src_i, road.dst_i] {
                        for cross in &map.get_i(*i).roads {
                            if !ids.contains(cross) {
                                cross_streets.insert(*cross);
                            }
                        }
                    }
                }
                Transition::Replace(Box::new(CrossStreet {
                    first: *ids.iter().next().unwrap(),
                    autocomplete: Autocomplete::new(
                        &format!("{} and what?", name),
                        cross_streets
                            .into_iter()
                            .map(|r| (map.get_r(r).get_name(), r))
                            .collect(),
                    ),
                }))
            }
            InputResult::StillActive => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.autocomplete.draw(g);
    }
}

struct CrossStreet {
    first: RoadID,
    autocomplete: Autocomplete<RoadID>,
}

impl State for CrossStreet {
    // When None, this is done.
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let map = &ui.primary.map;
        match self.autocomplete.event(ctx.input) {
            InputResult::Canceled => {
                // Just warp to somewhere on the first road
                let road = map.get_r(self.first);
                println!("Warping to {}", road.get_name());
                Transition::ReplaceWithMode(
                    Box::new(Warping {
                        warper: Warper::new(
                            ctx,
                            road.center_pts.dist_along(road.center_pts.length() / 2.0).0,
                            None,
                        ),
                        id: Some(ID::Lane(road.all_lanes()[0])),
                    }),
                    EventLoopMode::Animation,
                )
            }
            InputResult::Done(name, ids) => {
                println!(
                    "Warping to {} and {}",
                    map.get_r(self.first).get_name(),
                    name
                );
                let road = map.get_r(*ids.iter().next().unwrap());
                let pt = if map.get_i(road.src_i).roads.contains(&self.first) {
                    map.get_i(road.src_i).polygon.center()
                } else {
                    map.get_i(road.dst_i).polygon.center()
                };
                Transition::ReplaceWithMode(
                    Box::new(Warping {
                        warper: Warper::new(ctx, pt, None),
                        id: Some(ID::Lane(road.all_lanes()[0])),
                    }),
                    EventLoopMode::Animation,
                )
            }
            InputResult::StillActive => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.autocomplete.draw(g);
    }
}

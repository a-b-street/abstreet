use crate::common::Warper;
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{Autocomplete, EventCtx, EventLoopMode, GfxCtx, InputResult};
use map_model::RoadID;
use std::collections::HashSet;

pub enum Navigator {
    FirstStreet(Autocomplete<RoadID>),
    CrossStreet(RoadID, Autocomplete<RoadID>),
    Warping(Warper),
}

impl Navigator {
    pub fn new(ui: &UI) -> Navigator {
        // TODO Canonicalize names, handling abbreviations like east/e and street/st
        Navigator::FirstStreet(Autocomplete::new(
            "Warp where?",
            ui.primary
                .map
                .all_roads()
                .iter()
                .map(|r| (r.get_name(), r.id))
                .collect(),
        ))
    }

    // When None, this is done.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        let map = &ui.primary.map;

        match self {
            Navigator::FirstStreet(autocomplete) => match autocomplete.event(ctx.input) {
                InputResult::Canceled => None,
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
                    *self = Navigator::CrossStreet(
                        *ids.iter().next().unwrap(),
                        Autocomplete::new(
                            &format!("{} and what?", name),
                            cross_streets
                                .into_iter()
                                .map(|r| (map.get_r(r).get_name(), r))
                                .collect(),
                        ),
                    );
                    Some(EventLoopMode::InputOnly)
                }
                InputResult::StillActive => Some(EventLoopMode::InputOnly),
            },
            Navigator::CrossStreet(first_id, autocomplete) => match autocomplete.event(ctx.input) {
                InputResult::Canceled => {
                    // Just warp to somewhere on the first road
                    let road = map.get_r(*first_id);
                    println!("Warping to {}", road.get_name());
                    *self = Navigator::Warping(Warper::new(
                        ctx,
                        road.center_pts.dist_along(road.center_pts.length() / 2.0).0,
                        ID::Lane(road.all_lanes()[0]),
                    ));
                    Some(EventLoopMode::Animation)
                }
                InputResult::Done(name, ids) => {
                    println!(
                        "Warping to {} and {}",
                        map.get_r(*first_id).get_name(),
                        name
                    );
                    let road = map.get_r(*ids.iter().next().unwrap());
                    let pt = if map.get_i(road.src_i).roads.contains(first_id) {
                        map.get_i(road.src_i).point
                    } else {
                        map.get_i(road.dst_i).point
                    };
                    *self = Navigator::Warping(Warper::new(ctx, pt, ID::Lane(road.all_lanes()[0])));
                    Some(EventLoopMode::Animation)
                }
                InputResult::StillActive => Some(EventLoopMode::InputOnly),
            },
            Navigator::Warping(ref warper) => warper.event(ctx, ui),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            Navigator::FirstStreet(ref autocomplete)
            | Navigator::CrossStreet(_, ref autocomplete) => {
                autocomplete.draw(g);
            }
            Navigator::Warping(_) => {}
        }
    }
}

use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{Autocomplete, EventCtx, EventLoopMode, GfxCtx, InputResult};
use geom::Line;
use map_model::RoadID;
use std::collections::HashSet;
use std::time::Instant;

// TODO Maybe pixels/second or something would be smoother
const ANIMATION_TIME_S: f64 = 0.5;

pub enum Navigator {
    // TODO Ask for a cross-street after the first one
    FirstStreet(Autocomplete<RoadID>),
    CrossStreet(RoadID, Autocomplete<RoadID>),
    Warping(Instant, Line),
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
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> Option<EventLoopMode> {
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
                    let pt = road.center_pts.dist_along(road.center_pts.length() / 2.0).0;
                    let at = ctx.canvas.center_to_map_pt();
                    if let Some(l) = Line::maybe_new(at, pt) {
                        *self = Navigator::Warping(Instant::now(), l);
                        Some(EventLoopMode::Animation)
                    } else {
                        None
                    }
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
                    let at = ctx.canvas.center_to_map_pt();
                    if let Some(l) = Line::maybe_new(at, pt) {
                        *self = Navigator::Warping(Instant::now(), l);
                        Some(EventLoopMode::Animation)
                    } else {
                        None
                    }
                }
                InputResult::StillActive => Some(EventLoopMode::InputOnly),
            },
            Navigator::Warping(started, line) => {
                // Weird to do stuff for any event?
                if ctx.input.nonblocking_is_update_event() {
                    ctx.input.use_update_event();
                }

                let percent = elapsed_seconds(*started) / ANIMATION_TIME_S;
                if percent >= 1.0 {
                    ctx.canvas.center_on_map_pt(line.pt2());
                    //ctx.primary.current_selection = Some(*id);
                    None
                } else {
                    ctx.canvas
                        .center_on_map_pt(line.dist_along(line.length() * percent));
                    Some(EventLoopMode::Animation)
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            Navigator::FirstStreet(ref autocomplete)
            | Navigator::CrossStreet(_, ref autocomplete) => {
                autocomplete.draw(g);
            }
            Navigator::Warping(_, _) => {}
        }
    }
}

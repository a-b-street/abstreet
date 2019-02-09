use crate::objects::{DrawCtx, ID};
use crate::plugins::{BlockingPlugin, PluginCtx};
use crate::render::DrawMap;
use abstutil::elapsed_seconds;
use ezgui::{EventLoopMode, GfxCtx, InputResult, TextBox};
use geom::{Line, Pt2D};
use map_model::{raw_data, AreaID, BuildingID, IntersectionID, LaneID, Map, ParcelID, RoadID};
use sim::{PedestrianID, Sim, TripID};
use std::time::Instant;
use std::usize;

// TODO Maybe pixels/second or something would be smoother
const ANIMATION_TIME_S: f64 = 0.5;

pub enum WarpState {
    EnteringSearch(TextBox),
    Warping(Instant, Line, ID),
}

impl WarpState {
    pub fn new(ctx: &mut PluginCtx) -> Option<WarpState> {
        if ctx.input.action_chosen("warp to an object") {
            return Some(WarpState::EnteringSearch(TextBox::new(
                "Warp to what?",
                None,
            )));
        }
        None
    }
}

impl BlockingPlugin for WarpState {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        match self {
            WarpState::EnteringSearch(tb) => match tb.event(ctx.input) {
                InputResult::Canceled => {
                    return false;
                }
                InputResult::Done(to, _) => {
                    if let Some((id, pt)) = warp_point(
                        to,
                        &ctx.primary.map,
                        &ctx.primary.sim,
                        &ctx.primary.draw_map,
                    ) {
                        let at = ctx.canvas.center_to_map_pt();
                        if let Some(l) = Line::maybe_new(at, pt) {
                            *self = WarpState::Warping(Instant::now(), l, id);
                        } else {
                            ctx.primary.current_selection = Some(id);
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                InputResult::StillActive => {}
            },
            WarpState::Warping(started, line, id) => {
                ctx.hints.mode = EventLoopMode::Animation;
                let percent = elapsed_seconds(*started) / ANIMATION_TIME_S;
                if percent >= 1.0 {
                    ctx.canvas.center_on_map_pt(line.pt2());
                    ctx.primary.current_selection = Some(*id);
                    return false;
                } else {
                    ctx.canvas
                        .center_on_map_pt(line.dist_along(line.length() * percent));
                }
            }
        };
        true
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &DrawCtx) {
        if let WarpState::EnteringSearch(tb) = self {
            tb.draw(g);
        }
    }
}

fn warp_point(line: String, map: &Map, sim: &Sim, draw_map: &DrawMap) -> Option<(ID, Pt2D)> {
    if line.is_empty() {
        return None;
    }

    let id = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let id = RoadID(idx);
                if let Some(r) = map.maybe_get_r(id) {
                    ID::Lane(r.children_forwards[0].0)
                } else {
                    warn!("{} doesn't exist", id);
                    return None;
                }
            }
            'l' => ID::Lane(LaneID(idx)),
            'i' => ID::Intersection(IntersectionID(idx)),
            'b' => ID::Building(BuildingID(idx)),
            'a' => ID::Area(AreaID(idx)),
            'P' => ID::Parcel(ParcelID(idx)),
            'p' => ID::Pedestrian(PedestrianID(idx)),
            'c' => {
                // This one gets more complicated. :)
                ID::Car(sim.lookup_car_id(idx)?)
            }
            't' => ID::Trip(TripID(idx)),
            'T' => {
                if let Some(id) = map.lookup_turn_by_idx(idx) {
                    ID::Turn(id)
                } else {
                    warn!("{} isn't a known TurnID", line);
                    return None;
                }
            }
            'I' => {
                let stable_id = raw_data::StableIntersectionID(idx);
                if let Some(i) = map
                    .all_intersections()
                    .iter()
                    .find(|i| i.stable_id == stable_id)
                {
                    ID::Intersection(i.id)
                } else {
                    warn!("{} isn't known", stable_id);
                    return None;
                }
            }
            'R' => {
                let stable_id = raw_data::StableRoadID(idx);
                if let Some(r) = map.all_roads().iter().find(|r| r.stable_id == stable_id) {
                    ID::Lane(r.children_forwards[0].0)
                } else {
                    warn!("{} isn't known", stable_id);
                    return None;
                }
            }
            _ => {
                warn!("{} isn't a valid ID; Should be [libepct][0-9]+", line);
                return None;
            }
        },
        Err(_) => {
            return None;
        }
    };
    if let Some(pt) = id.canonical_point(map, sim, draw_map) {
        info!("Warping to {:?}", id);
        Some((id, pt))
    } else {
        warn!("{:?} doesn't exist", id);
        None
    }
}

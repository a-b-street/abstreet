use abstutil::elapsed_seconds;
use ezgui::{GfxCtx, InputResult, TextBox};
use geom::{Line, Pt2D};
use map_model::{AreaID, BuildingID, IntersectionID, LaneID, Map, ParcelID, RoadID};
use objects::{Ctx, DEBUG, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::DrawMap;
use sim::{CarID, PedestrianID, Sim, TripID};
use std::time::Instant;
use std::usize;

// TODO Maybe pixels/second or something would be smoother
const ANIMATION_TIME_S: f64 = 0.5;

pub enum WarpState {
    Empty,
    EnteringSearch(TextBox),
    Warping(Instant, Line, ID),
}

impl WarpState {
    pub fn new() -> WarpState {
        WarpState::Empty
    }
}

impl Plugin for WarpState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let mut new_state: Option<WarpState> = None;
        match self {
            WarpState::Empty => {
                if ctx.input.unimportant_key_pressed(
                    Key::J,
                    DEBUG,
                    "start searching for something to warp to",
                ) {
                    new_state = Some(WarpState::EnteringSearch(TextBox::new(
                        "Warp to what?",
                        None,
                    )));
                }
            }
            WarpState::EnteringSearch(tb) => match tb.event(ctx.input) {
                InputResult::Canceled => {
                    new_state = Some(WarpState::Empty);
                }
                InputResult::Done(to, _) => {
                    if let Some((id, pt)) = warp_point(
                        to,
                        &ctx.primary.map,
                        &ctx.primary.sim,
                        &ctx.primary.draw_map,
                    ) {
                        new_state = Some(WarpState::Warping(
                            Instant::now(),
                            Line::new(ctx.canvas.center_to_map_pt(), pt),
                            id,
                        ));
                    } else {
                        new_state = Some(WarpState::Empty);
                    }
                }
                InputResult::StillActive => {}
            },
            WarpState::Warping(started, line, id) => {
                ctx.osd.animation_mode();
                let percent = elapsed_seconds(*started) / ANIMATION_TIME_S;
                if percent >= 1.0 {
                    ctx.canvas.center_on_map_pt(line.pt2());
                    ctx.primary.current_selection = Some(*id);
                    new_state = Some(WarpState::Empty);
                } else {
                    ctx.canvas
                        .center_on_map_pt(line.dist_along(percent * line.length()));
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            WarpState::Empty => false,
            _ => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let WarpState::EnteringSearch(tb) = self {
            tb.draw(g, ctx.canvas);
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
            // TODO ideally "pa" prefix?
            'e' => ID::Parcel(ParcelID(idx)),
            'p' => ID::Pedestrian(PedestrianID(idx)),
            'c' => ID::Car(CarID(idx)),
            't' => ID::Trip(TripID(idx)),
            // TODO "tu"?
            'u' => {
                if let Some(id) = map.lookup_turn_by_idx(idx) {
                    ID::Turn(id)
                } else {
                    warn!("{} isn't a known TurnID", line);
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

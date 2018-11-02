use ezgui::{Canvas, GfxCtx, InputResult, TextBox};
use map_model::{AreaID, BuildingID, IntersectionID, LaneID, Map, ParcelID, RoadID};
use objects::{Ctx, DEBUG, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::DrawMap;
use sim::{CarID, PedestrianID, Sim, TripID};
use std::usize;

pub enum WarpState {
    Empty,
    EnteringSearch(TextBox),
}

impl WarpState {
    pub fn new() -> WarpState {
        WarpState::Empty
    }
}

impl Plugin for WarpState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, map, sim, draw_map, canvas, selected) = (
            ctx.input,
            &ctx.primary.map,
            &ctx.primary.sim,
            &ctx.primary.draw_map,
            ctx.canvas,
            &mut ctx.primary.current_selection,
        );

        let mut new_state: Option<WarpState> = None;
        match self {
            WarpState::Empty => {
                if input.unimportant_key_pressed(
                    Key::J,
                    DEBUG,
                    "start searching for something to warp to",
                ) {
                    new_state = Some(WarpState::EnteringSearch(TextBox::new("Warp to what?")));
                }
            }
            WarpState::EnteringSearch(tb) => match tb.event(input) {
                InputResult::Canceled => {
                    new_state = Some(WarpState::Empty);
                }
                InputResult::Done(to, _) => {
                    warp(to, map, sim, draw_map, canvas, selected);
                    new_state = Some(WarpState::Empty);
                }
                InputResult::StillActive => {}
            },
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

fn warp(
    line: String,
    map: &Map,
    sim: &Sim,
    draw_map: &DrawMap,
    canvas: &mut Canvas,
    selected: &mut Option<ID>,
) {
    if line.is_empty() {
        return;
    }

    let id = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let id = RoadID(idx);
                if let Some(r) = map.maybe_get_r(id) {
                    ID::Lane(r.children_forwards[0].0)
                } else {
                    warn!("{} doesn't exist", id);
                    return;
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
            _ => {
                warn!("{} isn't a valid ID; Should be [libepct][0-9]+", line);
                return;
            }
        },
        Err(_) => {
            return;
        }
    };
    if let Some(pt) = id.canonical_point(map, sim, draw_map) {
        info!("Warping to {:?}", id);
        *selected = Some(id);
        canvas.center_on_map_pt(pt);
    } else {
        warn!("{:?} doesn't exist", id);
    }
}

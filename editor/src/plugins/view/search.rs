use crate::objects::{DrawCtx, ID};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx, InputResult, TextBox};

pub enum SearchState {
    EnteringSearch(TextBox),
    FilterOSM(String),
}

impl SearchState {
    pub fn new(ctx: &mut PluginCtx) -> Option<SearchState> {
        if ctx.input.action_chosen("search for something") {
            return Some(SearchState::EnteringSearch(TextBox::new(
                "Search for what?",
                None,
            )));
        }
        None
    }

    // If not, act like stackable modal.
    pub fn is_blocking(&self) -> bool {
        match self {
            SearchState::EnteringSearch(_) => true,
            SearchState::FilterOSM(_) => false,
        }
    }
}

impl Plugin for SearchState {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        match self {
            SearchState::EnteringSearch(tb) => match tb.event(&mut ctx.input) {
                InputResult::Canceled => {
                    return false;
                }
                InputResult::Done(filter, _) => {
                    *self = SearchState::FilterOSM(filter);
                }
                InputResult::StillActive => {}
            },
            SearchState::FilterOSM(filter) => {
                ctx.input.set_mode_with_prompt(
                    "Search",
                    format!("Search for {}", filter),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("quit") {
                    return false;
                }
            }
        };
        true
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &DrawCtx) {
        if let SearchState::EnteringSearch(tb) = self {
            tb.draw(g);
        }
    }

    fn color_for(&self, obj: ID, ctx: &DrawCtx) -> Option<Color> {
        if let SearchState::FilterOSM(filter) = self {
            let osm_tags = match obj {
                ID::Lane(l) => &ctx.map.get_parent(l).osm_tags,
                ID::Building(b) => &ctx.map.get_b(b).osm_tags,
                _ => {
                    return None;
                }
            };
            for (k, v) in osm_tags {
                if format!("{}={}", k, v).contains(filter) {
                    return Some(ctx.cs.get_def("search result", Color::RED));
                }
            }
        }
        None
    }
}

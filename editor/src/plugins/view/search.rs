use crate::objects::{Ctx, DEBUG_EXTRA, ID};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx, InputResult, TextBox};
use piston::input::Key;

pub enum SearchState {
    EnteringSearch(TextBox),
    FilterOSM(String),
}

impl SearchState {
    pub fn new(ctx: &mut PluginCtx) -> Option<SearchState> {
        if ctx
            .input
            .unimportant_key_pressed(Key::Slash, DEBUG_EXTRA, "start searching")
        {
            return Some(SearchState::EnteringSearch(TextBox::new(
                "Search for what?",
                None,
            )));
        }
        None
    }

    pub fn is_blocking(&self) -> bool {
        match self {
            SearchState::EnteringSearch(_) => true,
            SearchState::FilterOSM(_) => false,
        }
    }
}

impl Plugin for SearchState {
    fn new_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let mut new_state: Option<SearchState> = None;
        match self {
            SearchState::EnteringSearch(tb) => match tb.event(&mut ctx.input) {
                InputResult::Canceled => {
                    return false;
                }
                InputResult::Done(filter, _) => {
                    new_state = Some(SearchState::FilterOSM(filter));
                }
                InputResult::StillActive => {}
            },
            SearchState::FilterOSM(filter) => {
                if ctx.input.key_pressed(
                    Key::Return,
                    &format!("clear the current search for {}", filter),
                ) {
                    return false;
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        true
    }

    fn new_draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        if let SearchState::EnteringSearch(tb) = self {
            tb.draw(g, ctx.canvas);
        }
    }

    fn new_color_for(&self, obj: ID, ctx: &mut Ctx) -> Option<Color> {
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
                    return Some(ctx.cs.get("search result", Color::RED));
                }
            }
        }
        None
    }
}

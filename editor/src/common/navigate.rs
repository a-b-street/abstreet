use crate::ui::UI;
use ezgui::{Autocomplete, EventCtx, EventLoopMode, GfxCtx, InputResult};
use map_model::RoadID;

pub enum Navigator {
    // TODO Ask for a cross-street after the first one
    Searching(Autocomplete<RoadID>),
}

impl Navigator {
    pub fn new(ui: &UI) -> Navigator {
        // TODO Canonicalize names, handling abbreviations like east/e and street/st
        Navigator::Searching(Autocomplete::new(
            "Warp to what?",
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
        match self {
            Navigator::Searching(autocomplete) => match autocomplete.event(ctx.input) {
                InputResult::Canceled => None,
                InputResult::Done(name, ids) => {
                    println!("Search for '{}' yielded {:?}", name, ids);
                    None
                }
                InputResult::StillActive => Some(EventLoopMode::InputOnly),
            },
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            Navigator::Searching(ref autocomplete) => {
                autocomplete.draw(g);
            }
        }
    }
}

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::Timer;
use map_gui::tools::{ChooseSomething, PopupMsg, PromptInput};
use widgetry::{Choice, EventCtx, State, Transition};

use crate::{App, BrowseNeighborhoods, ModalFilters, Partitioning};

/// Captures all of the edits somebody makes to a map in the LTN tool. Note this separate from
/// `map_model::MapEdits`.
///
/// TODO Note this format isn't future-proof at all. Changes to the LTN blockfinding algorithm or
/// map data (like RoadIDs) will probably break someone's edits.
#[derive(Serialize, Deserialize)]
pub struct Proposal {
    pub map: MapName,
    pub name: String,
    pub abst_version: String,

    pub partitioning: Partitioning,
    pub modal_filters: ModalFilters,
}

impl Proposal {
    pub fn save_ui(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        PromptInput::new_state(
            ctx,
            "Name this proposal",
            String::new(),
            Box::new(|name, _, app| {
                Self::save(app, name);
                Transition::Pop
            }),
        )
    }

    fn save(app: &App, name: String) {
        let path = abstio::path_ltn_proposals(app.map.get_name(), &name);
        let proposal = Proposal {
            map: app.map.get_name().clone(),
            name,
            abst_version: map_gui::tools::version().to_string(),

            partitioning: app.session.partitioning.clone(),
            modal_filters: app.session.modal_filters.clone(),
        };
        abstio::write_binary(path, &proposal);
    }

    pub fn load_picker_ui(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        ChooseSomething::new_state(
            ctx,
            "Load which proposal?",
            Choice::strings(abstio::list_all_objects(abstio::path_all_ltn_proposals(
                app.map.get_name(),
            ))),
            Box::new(|name, ctx, app| {
                Transition::Replace(match Self::load(ctx, app, &name) {
                    Some(err_state) => err_state,
                    None => BrowseNeighborhoods::new_state(ctx, app),
                })
            }),
        )
    }

    /// Try to load a proposal. If it fails, returns a popup message state.
    pub fn load(ctx: &mut EventCtx, app: &mut App, name: &str) -> Option<Box<dyn State<App>>> {
        ctx.loading_screen(
            "load existing proposal",
            |ctx, mut timer| match Self::inner_load(ctx, app, name, &mut timer) {
                Ok(()) => None,
                Err(err) => Some(PopupMsg::new_state(
                    ctx,
                    "Error",
                    vec![format!("Couldn't load proposal {}", name), err.to_string()],
                )),
            },
        )
    }

    fn inner_load(ctx: &mut EventCtx, app: &mut App, name: &str, timer: &mut Timer) -> Result<()> {
        let proposal: Proposal =
            abstio::maybe_read_binary(abstio::path_ltn_proposals(app.map.get_name(), name), timer)?;
        // TODO We could try to detect if the file still matches this version of the map or not
        app.session.partitioning = proposal.partitioning;
        app.session.modal_filters = proposal.modal_filters;
        app.session.draw_all_filters = app.session.modal_filters.draw(ctx, &app.map);
        Ok(())
    }
}

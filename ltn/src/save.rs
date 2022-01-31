use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::Timer;

use crate::{App, ModalFilters, Partitioning};

/// Captures all of the edits somebody makes to a map in the LTN tool. Note this separate from
/// `map_model::MapEdits`.
///
/// TODO Note this format isn't future-proof at all. Changes to the LTN blockfinding algorithm or
/// map data (like RoadIDs) will probably break someone's edits.
#[derive(Serialize, Deserialize)]
pub struct Proposal {
    pub map: MapName,
    pub name: String,

    pub partitioning: Partitioning,
    pub modal_filters: ModalFilters,
}

impl Proposal {
    pub fn save(app: &App, name: String) {
        let path = abstio::path_ltn_proposals(app.map.get_name(), &name);
        let proposal = Proposal {
            map: app.map.get_name().clone(),
            name,

            partitioning: app.session.partitioning.clone(),
            modal_filters: app.session.modal_filters.clone(),
        };
        abstio::write_binary(path, &proposal);
    }

    pub fn load(app: &mut App, name: &str, timer: &mut Timer) -> Result<()> {
        let proposal: Proposal =
            abstio::maybe_read_binary(abstio::path_ltn_proposals(app.map.get_name(), name), timer)?;
        // TODO We could try to detect if the file still matches this version of the map or not
        app.session.partitioning = proposal.partitioning;
        app.session.modal_filters = proposal.modal_filters;
        Ok(())
    }
}

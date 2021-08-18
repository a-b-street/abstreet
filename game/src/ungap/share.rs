use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use map_gui::load::FutureLoader;
use map_gui::tools::PopupMsg;
use widgetry::{EventCtx, State};

use crate::app::{App, Transition};

pub const PROPOSAL_HOST_URL: &str = "http://localhost:8080/v1";
//pub const PROPOSAL_HOST_URL: &str = "https://aorta-routes.appspot.com/v1";

pub fn upload_proposal(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
    let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
    let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
    let edits_json = abstutil::to_json(&app.primary.map.get_edits().to_permanent(&app.primary.map));
    FutureLoader::<App, String>::new_state(
        ctx,
        Box::pin(async move {
            // We don't really need this ID from the API; it's the md5sum.
            let id = abstio::http_post(format!("{}/create", PROPOSAL_HOST_URL), edits_json).await?;
            // TODO I'm so lost in this type magic
            let wrapper: Box<dyn Send + FnOnce(&App) -> String> = Box::new(move |_| id);
            Ok(wrapper)
        }),
        outer_progress_rx,
        inner_progress_rx,
        "Uploading proposal",
        Box::new(|ctx, _, result| {
            Transition::Replace(match result {
                Ok(id) => {
                    info!("Proposal uploaded! {}/get?id={}", PROPOSAL_HOST_URL, id);
                    UploadedProposals::proposal_uploaded(id);
                    // TODO Change URL
                    // TODO Ahh this doesn't actually remake the top panel and change the share
                    // button. Grrrr.
                    PopupMsg::new_state(ctx, "Success", vec!["You can now share the URL..."])
                }
                Err(err) => PopupMsg::new_state(
                    ctx,
                    "Failure",
                    vec![format!("Couldn't upload proposal: {}", err)],
                ),
            })
        }),
    )
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UploadedProposals {
    pub md5sums: BTreeSet<String>,
}

impl UploadedProposals {
    pub fn load() -> UploadedProposals {
        abstio::maybe_read_json::<UploadedProposals>(
            abstio::path_player("uploaded_proposals.json"),
            &mut Timer::throwaway(),
        )
        .unwrap_or_else(|_| UploadedProposals {
            md5sums: BTreeSet::new(),
        })
    }

    pub fn should_upload_proposal(app: &App) -> bool {
        let map = &app.primary.map;
        if map.get_edits().commands.is_empty() {
            return false;
        }
        let checksum = map.get_edits().get_checksum(map);
        !UploadedProposals::load().md5sums.contains(&checksum)
    }

    fn proposal_uploaded(checksum: String) {
        let mut uploaded = UploadedProposals::load();
        uploaded.md5sums.insert(checksum);
        abstio::write_json(abstio::path_player("uploaded_proposals.json"), &uploaded);
    }
}

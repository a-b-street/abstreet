use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use map_gui::tools::grey_out_map;
use widgetry::tools::URLManager;
use widgetry::tools::{open_browser, FutureLoader, PopupMsg};
use widgetry::{
    DrawBaselayer, EventCtx, GfxCtx, Key, Line, Panel, SimpleState, State, Text, TextExt, Widget,
};

use crate::{App, Transition};

pub const PROPOSAL_HOST_URL: &str = "https://aorta-routes.appspot.com/v1";

pub struct ShareProposal {
    url: Option<String>,
}

impl ShareProposal {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let checksum = match app.per_map.proposals.get_current().checksum(app) {
            Ok(checksum) => checksum,
            Err(err) => {
                return PopupMsg::new_state(
                    ctx,
                    "Error",
                    vec![format!("Can't save this proposal: {}", err)],
                );
            }
        };

        let mut url = None;
        let mut col = vec![Widget::row(vec![
            Line("Share this proposal").small_heading().into_widget(ctx),
            ctx.style().btn_close_widget(ctx),
        ])];
        if UploadedProposals::load().md5sums.contains(&checksum) {
            let map_path = app
                .per_map
                .map
                .get_name()
                .path()
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string();
            let consultation = if let Some(ref x) = app.per_map.consultation_id {
                format!("&--consultation={x}")
            } else {
                String::new()
            };
            url = Some(format!(
                "https://play.abstreet.org/{}/ltn.html?{}&--proposal=remote/{}{}",
                map_gui::tools::version(),
                map_path,
                checksum,
                consultation
            ));

            if cfg!(target_arch = "wasm32") {
                col.push("Proposal uploaded! Share your browser's URL.".text_widget(ctx));
            } else {
                col.push("Proposal uploaded! Share the URL below.".text_widget(ctx));
            }
            col.push(
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text(url.as_ref().unwrap())
                    .build_widget(ctx, "open in browser"),
            );

            if cfg!(target_arch = "wasm32") {
                col.push(ctx.style().btn_plain.text("Back").build_def(ctx));
            } else {
                col.push(Widget::row(vec![
                    ctx.style()
                        .btn_solid_primary
                        .text("Copy URL to clipboard")
                        .build_def(ctx),
                    ctx.style().btn_plain.text("Back").build_def(ctx),
                ]));
            }
        } else {
            let mut txt = Text::new();
            // The Creative Commons licenses all require attribution, but we have no user accounts
            // or ways of proving identity yet!
            txt.add_line(Line(
                "You'll upload this proposal anonymously, in the public domain",
            ));
            txt.add_line(Line("You can't delete or edit it after uploading"));
            txt.add_line(Line(
                "(But you can upload and share new versions of the proposal)",
            ));
            col.push(txt.into_widget(ctx));
            col.push(Widget::row(vec![
                ctx.style()
                    .btn_solid_primary
                    .text("Upload")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
                ctx.style().btn_plain.text("Cancel").build_def(ctx),
            ]));
        }

        let panel = Panel::new_builder(Widget::col(col)).build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(ShareProposal { url }))
    }
}

impl SimpleState<App> for ShareProposal {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        _: &mut Panel,
    ) -> Transition {
        match x {
            "close" | "Cancel" | "Back" => Transition::Pop,
            "Upload" => {
                let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
                let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
                let proposal_contents = app
                    .per_map
                    .proposals
                    .get_current()
                    .to_gzipped_bytes(app)
                    .unwrap();
                return Transition::Replace(FutureLoader::<App, String>::new_state(
                    ctx,
                    Box::pin(async move {
                        // We don't really need this ID from the API; it's the md5sum.
                        let id = abstio::http_post(
                            format!("{}/create-ltn", PROPOSAL_HOST_URL),
                            proposal_contents,
                        )
                        .await?;
                        // TODO I'm so lost in this type magic
                        let wrapper: Box<dyn Send + FnOnce(&App) -> String> = Box::new(move |_| id);
                        Ok(wrapper)
                    }),
                    outer_progress_rx,
                    inner_progress_rx,
                    "Uploading proposal",
                    Box::new(move |ctx, app, result| match result {
                        Ok(id) => {
                            URLManager::update_url_param(
                                "--proposal".to_string(),
                                format!("remote/{}", id),
                            );
                            info!("Proposal uploaded! {}/get-ltn?id={}", PROPOSAL_HOST_URL, id);
                            UploadedProposals::proposal_uploaded(id);
                            Transition::Replace(ShareProposal::new_state(ctx, app))
                        }
                        Err(err) => Transition::Multi(vec![
                            Transition::Pop,
                            Transition::Push(ShareProposal::new_state(ctx, app)),
                            Transition::Push(PopupMsg::new_state(
                                ctx,
                                "Failure",
                                vec![format!("Couldn't upload proposal: {}", err)],
                            )),
                        ]),
                    }),
                ));
            }
            "Copy URL to clipboard" => {
                widgetry::tools::set_clipboard(self.url.clone().unwrap());
                Transition::Keep
            }
            "open in browser" => {
                open_browser(self.url.as_ref().unwrap());
                Transition::Keep
            }
            _ => unreachable!(),
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UploadedProposals {
    pub md5sums: BTreeSet<String>,
}

impl UploadedProposals {
    pub fn load() -> UploadedProposals {
        abstio::maybe_read_json::<UploadedProposals>(
            abstio::path_player("uploaded_ltn_proposals.json"),
            &mut Timer::throwaway(),
        )
        .unwrap_or_else(|_| UploadedProposals {
            md5sums: BTreeSet::new(),
        })
    }

    fn proposal_uploaded(checksum: String) {
        let mut uploaded = UploadedProposals::load();
        uploaded.md5sums.insert(checksum);
        abstio::write_json(
            abstio::path_player("uploaded_ltn_proposals.json"),
            &uploaded,
        );
    }
}

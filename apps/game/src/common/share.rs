use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use map_gui::tools::grey_out_map;
use widgetry::tools::URLManager;
use widgetry::tools::{open_browser, FutureLoader, PopupMsg};
use widgetry::{EventCtx, GfxCtx, Key, Line, Panel, SimpleState, State, Text, TextExt, Widget};

use crate::app::{App, Transition};

//pub const PROPOSAL_HOST_URL: &str = "http://localhost:8080/v1";
pub const PROPOSAL_HOST_URL: &str = "https://aorta-routes.appspot.com/v1";

pub struct ShareProposal {
    url: Option<String>,
    url_flag: &'static str,
}

impl ShareProposal {
    /// This will point to a URL with the new edits and the current map, but the caller needs to
    /// indicate a flag to reach the proper mode of A/B Street.
    pub fn new_state(ctx: &mut EventCtx, app: &App, url_flag: &'static str) -> Box<dyn State<App>> {
        let checksum = app.primary.map.get_edits().get_checksum(&app.primary.map);
        let mut url = None;
        let mut col = vec![Widget::row(vec![
            Line("Share this proposal").small_heading().into_widget(ctx),
            ctx.style().btn_close_widget(ctx),
        ])];
        if UploadedProposals::load().md5sums.contains(&checksum) {
            let map_path = app
                .primary
                .map
                .get_name()
                .path()
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string();
            url = Some(format!(
                "http://play.abstreet.org/{}/abstreet.html?{}&{}&--edits=remote/{}",
                map_gui::tools::version(),
                url_flag,
                map_path,
                checksum
            ));

            if cfg!(target_arch = "wasm32") {
                col.push("Proposal uploaded! Share your browser's URL".text_widget(ctx));
            } else {
                col.push(
                    ctx.style()
                        .btn_plain
                        .btn()
                        .label_underlined_text(url.as_ref().unwrap())
                        .build_widget(ctx, "open in browser"),
                );
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
        <dyn SimpleState<_>>::new_state(panel, Box::new(ShareProposal { url, url_flag }))
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
                let edits_json =
                    abstutil::to_json(&app.primary.map.get_edits().to_permanent(&app.primary.map));
                let url_flag = self.url_flag;
                return Transition::Replace(FutureLoader::<App, String>::new_state(
                    ctx,
                    Box::pin(async move {
                        // We don't really need this ID from the API; it's the md5sum.
                        let id =
                            abstio::http_post(format!("{}/create", PROPOSAL_HOST_URL), edits_json)
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
                                "--edits".to_string(),
                                format!("remote/{}", id),
                            );
                            info!("Proposal uploaded! {}/get?id={}", PROPOSAL_HOST_URL, id);
                            UploadedProposals::proposal_uploaded(id);
                            Transition::Replace(ShareProposal::new_state(ctx, app, url_flag))
                        }
                        Err(err) => Transition::Multi(vec![
                            Transition::Pop,
                            Transition::Push(ShareProposal::new_state(ctx, app, url_flag)),
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
                set_clipboard(self.url.clone().unwrap());
                Transition::Keep
            }
            "open in browser" => {
                open_browser(self.url.as_ref().unwrap());
                Transition::Keep
            }
            _ => unreachable!(),
        }
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
            abstio::path_player("uploaded_proposals.json"),
            &mut Timer::throwaway(),
        )
        .unwrap_or_else(|_| UploadedProposals {
            md5sums: BTreeSet::new(),
        })
    }

    fn proposal_uploaded(checksum: String) {
        let mut uploaded = UploadedProposals::load();
        uploaded.md5sums.insert(checksum);
        abstio::write_json(abstio::path_player("uploaded_proposals.json"), &uploaded);
    }
}

fn set_clipboard(x: String) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use clipboard::{ClipboardContext, ClipboardProvider};
        if let Err(err) =
            ClipboardProvider::new().and_then(|mut ctx: ClipboardContext| ctx.set_contents(x))
        {
            error!("Copying to clipboard broke: {}", err);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = x;
    }
}

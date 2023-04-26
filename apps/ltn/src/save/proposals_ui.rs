use map_gui::tools::{FilePicker, FileSaver, FileSaverContents};
use widgetry::tools::{ChooseSomething, PopupMsg};
use widgetry::{lctrl, Choice, EventCtx, Key, MultiKey, State, Widget};

use super::save_dialog::SaveDialog;
use super::share::ShareProposal;
use super::{stash_current_proposal, PreserveState, Proposal, Proposals};
use crate::{App, Transition};

impl Proposals {
    pub fn to_widget_expanded(&self, ctx: &EventCtx, app: &App) -> Widget {
        let mut col = Vec::new();
        for (action, icon, hotkey) in [
            ("New", "pencil", None),
            ("Load (quick)", "folder", None),
            ("Save (quick)", "save", Some(MultiKey::from(lctrl(Key::S)))),
            ("Load (file)", "folder", None),
            ("Save (file)", "save", None),
            ("Share", "share", None),
            ("Export GeoJSON", "export", None),
        ] {
            col.push(
                ctx.style()
                    .btn_plain
                    .icon_text(&format!("system/assets/tools/{icon}.svg"), action)
                    .hotkey(hotkey)
                    .build_def(ctx),
            );
        }

        for (idx, proposal) in self.list.iter().enumerate() {
            let button = if let Some(proposal) = proposal {
                ctx.style()
                    .btn_solid_primary
                    .text(format!("{} - {}", idx + 1, proposal.name))
                    .hotkey(Key::NUM_KEYS[idx])
                    .build_widget(ctx, &format!("switch to proposal {}", idx))
            } else {
                ctx.style()
                    .btn_solid_primary
                    .text(format!(
                        "{} - {}",
                        idx + 1,
                        app.per_map.proposals.current_proposal.name
                    ))
                    .disabled(true)
                    .build_def(ctx)
            };
            col.push(Widget::row(vec![
                button,
                // The first proposal (usually "existing LTNs", unless we're in a special consultation
                // mode) is special and can't ever be removed
                if idx != 0 {
                    ctx.style()
                        .btn_close()
                        .disabled(self.list.len() == 1)
                        .build_widget(ctx, &format!("hide proposal {}", idx))
                } else {
                    Widget::nothing()
                },
            ]));
            // If somebody tries to load too many proposals, just stop
            if idx == 9 {
                break;
            }
        }
        Widget::col(col)
    }

    pub fn to_widget_collapsed(&self, ctx: &EventCtx) -> Widget {
        let mut col = Vec::new();
        for (action, icon) in [
            ("New", "pencil"),
            ("Load (quick)", "folder"),
            ("Save (quick)", "save"),
            ("Share", "share"),
            ("Export GeoJSON", "export"),
        ] {
            col.push(
                ctx.style()
                    .btn_plain
                    .icon(&format!("system/assets/tools/{icon}.svg"))
                    .build_widget(ctx, action),
            );
        }
        Widget::col(col)
    }

    pub fn handle_action(
        ctx: &mut EventCtx,
        app: &mut App,
        preserve_state: &PreserveState,
        action: &str,
    ) -> Option<Transition> {
        match action {
            "New" => {
                // Fork a new proposal from the first one
                if app.per_map.proposals.current != 0 {
                    switch_to_existing_proposal(ctx, app, 0);
                }

                app.per_map.proposals.before_edit();
            }
            "Load (quick)" => {
                return Some(Transition::Push(load_picker_ui(
                    ctx,
                    app,
                    preserve_state.clone(),
                )));
            }
            "Save (quick)" => {
                return Some(Transition::Push(SaveDialog::new_state(
                    ctx,
                    app,
                    preserve_state.clone(),
                )));
            }
            "Load (file)" => {
                let preserve_state = preserve_state.clone();
                return Some(Transition::Push(FilePicker::new_state(
                    ctx,
                    Some(abstio::path_all_ltn_proposals(app.per_map.map.get_name())),
                    Box::new(move |ctx, app, maybe_file| {
                        match maybe_file {
                            Ok(Some((path, bytes))) => {
                                match Proposal::load_from_bytes(ctx, app, &path, Ok(bytes)) {
                                    Some(err_state) => Transition::Replace(err_state),
                                    None => preserve_state.switch_to_state(ctx, app),
                                }
                            }
                            // No file chosen, just quit the picker
                            Ok(None) => Transition::Pop,
                            Err(err) => Transition::Replace(PopupMsg::new_state(
                                ctx,
                                "Error",
                                vec![err.to_string()],
                            )),
                        }
                    }),
                )));
            }
            "Save (file)" => {
                let proposal = &app.per_map.proposals.current_proposal;

                return Some(Transition::Push(match proposal.to_gzipped_bytes(app) {
                    Ok(contents) => FileSaver::with_default_messages(
                        ctx,
                        // * is used to indicate an unsaved file; don't include it in the filename
                        format!("{}.json.gz", proposal.name.replace("*", "")),
                        Some(abstio::path_all_ltn_proposals(app.per_map.map.get_name())),
                        FileSaverContents::Bytes(contents),
                    ),
                    Err(err) => PopupMsg::new_state(ctx, "Save failed", vec![err.to_string()]),
                }));
            }
            "Share" => {
                return Some(Transition::Push(ShareProposal::new_state(ctx, app)));
            }
            "Export GeoJSON" => {
                return Some(Transition::Push(match crate::export::geojson_string(app) {
                    Ok(contents) => FileSaver::with_default_messages(
                        ctx,
                        format!("ltn_{}.geojson", app.per_map.map.get_name().map),
                        None,
                        FileSaverContents::String(contents),
                    ),
                    Err(err) => PopupMsg::new_state(ctx, "Export failed", vec![err.to_string()]),
                }));
            }
            _ => {
                if let Some(x) = action.strip_prefix("switch to proposal ") {
                    let idx = x.parse::<usize>().unwrap();
                    switch_to_existing_proposal(ctx, app, idx);
                } else if let Some(x) = action.strip_prefix("hide proposal ") {
                    let idx = x.parse::<usize>().unwrap();
                    if idx == app.per_map.proposals.current {
                        // First make sure we're not hiding the current proposal
                        switch_to_existing_proposal(ctx, app, if idx == 0 { 1 } else { idx - 1 });
                    }

                    // Remove it
                    app.per_map.proposals.list.remove(idx);

                    // Fix up indices
                    if idx < app.per_map.proposals.current {
                        app.per_map.proposals.current -= 1;
                    }
                } else {
                    return None;
                }
            }
        }

        Some(preserve_state.clone().switch_to_state(ctx, app))
    }
}

fn switch_to_existing_proposal(ctx: &mut EventCtx, app: &mut App, idx: usize) {
    stash_current_proposal(app);

    let proposal = app
        .per_map
        .proposals
        .list
        .get_mut(idx)
        .unwrap()
        .take()
        .unwrap();
    app.per_map.proposals.current = idx;

    proposal.make_active(ctx, app);
}

fn load_picker_ui(
    ctx: &mut EventCtx,
    app: &App,
    preserve_state: PreserveState,
) -> Box<dyn State<App>> {
    // Don't bother trying to filter out proposals currently loaded -- by loading twice, somebody
    // effectively makes a copy to modify a bit
    ChooseSomething::new_state(
        ctx,
        "Load which proposal?",
        // basename (and thus list_all_objects) turn "foo.json.gz" into "foo.json", so further
        // strip out the extension.
        // TODO Fix basename, but make sure nothing downstream breaks
        Choice::strings(
            abstio::list_all_objects(abstio::path_all_ltn_proposals(app.per_map.map.get_name()))
                .into_iter()
                .map(abstutil::basename)
                .collect(),
        ),
        Box::new(move |name, ctx, app| {
            match Proposal::load_from_path(
                ctx,
                app,
                abstio::path_ltn_proposals(app.per_map.map.get_name(), &name),
            ) {
                Some(err_state) => Transition::Replace(err_state),
                None => preserve_state.switch_to_state(ctx, app),
            }
        }),
    )
}

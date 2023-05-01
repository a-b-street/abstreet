use map_gui::tools::{FilePicker, FileSaver, FileSaverContents};
use widgetry::tools::{ChooseSomething, PopupMsg};
use widgetry::{lctrl, Choice, EventCtx, Key, MultiKey, State, Widget};

use super::save_dialog::SaveDialog;
use super::share::ShareProposal;
use super::{stash_current_proposal, PreserveState, Proposal, Proposals};
use crate::{App, Transition};

impl Proposals {
    pub fn to_widget_expanded(&self, ctx: &EventCtx) -> Widget {
        let mut col = Vec::new();
        for (action, icon, hotkey) in [
            ("New", "pencil", None),
            ("Load", "folder", None),
            ("Save", "save", Some(MultiKey::from(lctrl(Key::S)))),
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
            let button = ctx
                .style()
                .btn_solid_primary
                .text(format!("{} - {}", idx + 1, proposal.edits.edits_name))
                .hotkey(Key::NUM_KEYS[idx])
                .disabled(idx == self.current)
                .build_widget(ctx, &format!("switch to proposal {}", idx));
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
            ("Load", "folder"),
            ("Save", "save"),
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
            }
            "Load" => {
                return Some(Transition::Push(load_picker_ui(
                    ctx,
                    app,
                    preserve_state.clone(),
                )));
            }
            "Save" => {
                return Some(Transition::Push(SaveDialog::new_state(
                    ctx,
                    app,
                    preserve_state.clone(),
                )));
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
    app.per_map.proposals.current = idx;
    app.apply_edits(app.per_map.proposals.list[idx].edits.clone());
    crate::redraw_all_filters(ctx, app);
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
        {
            let mut choices = vec!["Load from file on your computer".to_string()];
            choices.extend(
                abstio::list_all_objects(abstio::path_all_ltn_proposals(
                    app.per_map.map.get_name(),
                ))
                .into_iter()
                .map(abstutil::basename),
            );
            Choice::strings(choices)
        },
        Box::new(move |name, ctx, app| {
            if name == "Load from file on your computer" {
                //let preserve_state = preserve_state.clone();
                Transition::Replace(FilePicker::new_state(
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
                ))
            } else {
                match Proposal::load_from_path(
                    ctx,
                    app,
                    abstio::path_ltn_proposals(app.per_map.map.get_name(), &name),
                ) {
                    Some(err_state) => Transition::Replace(err_state),
                    None => preserve_state.switch_to_state(ctx, app),
                }
            }
        }),
    )
}

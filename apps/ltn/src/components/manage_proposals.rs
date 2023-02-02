impl Proposals {
    pub fn to_widget_expanded(&self, ctx: &EventCtx, app: &App) -> Widget {
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

                app.per_map.proposals.before_edit();
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
                return Some(Transition::Push(share::ShareProposal::new_state(ctx, app)));
            }
            "Export GeoJSON" => {
                let result = crate::export::write_geojson_file(app);
                return Some(Transition::Push(match result {
                    Ok(path) => PopupMsg::new_state(
                        ctx,
                        "LTNs exported",
                        vec![format!("Data exported to {}", path)],
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

use std::collections::HashMap;

use geom::Percent;
use map_gui::load::MapLoader;
use map_model::PermanentMapEdits;
use synthpop::Scenario;
use widgetry::tools::{open_browser, PopupMsg};
use widgetry::{EventCtx, Key, Line, Panel, SimpleState, State, Text, Widget};

use crate::app::{App, Transition};
use crate::edit::apply_map_edits;
use crate::sandbox::{GameplayMode, SandboxMode};

pub struct Proposals {
    proposals: HashMap<String, PermanentMapEdits>,
    current: Option<String>,
}

impl Proposals {
    pub fn new_state(ctx: &mut EventCtx, current: Option<String>) -> Box<dyn State<App>> {
        let mut proposals = HashMap::new();
        let mut tab_buttons = Vec::new();
        let mut current_tab_rows = Vec::new();
        // If a proposal has fallen out of date, it'll be skipped with an error logged. Since these
        // are under version control, much more likely to notice when they break (or we could add a
        // step to data/regen.sh).
        for (name, edits) in
            abstio::load_all_objects::<PermanentMapEdits>(abstio::path("system/proposals"))
        {
            if current == Some(name.clone()) {
                let mut txt = Text::new();
                txt.add_line(Line(edits.get_title()).small_heading());
                for l in edits.proposal_description.iter().skip(1) {
                    txt.add_line(l);
                }
                current_tab_rows.push(
                    txt.wrap_to_pct(ctx, 70)
                        .into_widget(ctx)
                        .margin_below(15)
                        .margin_above(15),
                );

                if edits.proposal_link.is_some() {
                    current_tab_rows.push(
                        ctx.style()
                            .btn_plain
                            .btn()
                            .label_underlined_text("Read detailed write-up")
                            .build_def(ctx)
                            .margin_below(10),
                    );
                }
                current_tab_rows.push(
                    ctx.style()
                        .btn_solid_primary
                        .text("Try out this proposal")
                        .hotkey(Key::Enter)
                        .build_def(ctx),
                );

                tab_buttons.push(
                    ctx.style()
                        .btn_tab
                        .text(edits.get_title())
                        .disabled(true)
                        .build_def(ctx)
                        .margin_below(10),
                );
            } else {
                let hotkey = Key::NUM_KEYS
                    .get(proposals.len())
                    .map(|key| widgetry::MultiKey::from(*key));
                tab_buttons.push(
                    ctx.style()
                        .btn_outline
                        .text(edits.get_title())
                        .no_tooltip()
                        .hotkey(hotkey)
                        .build_widget(ctx, &name)
                        .margin_below(10),
                );
            }

            proposals.insert(name, edits);
        }

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Community proposals").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            {
                let mut txt =
                    Text::from("These are proposed changes to Seattle made by community members.");
                txt.add_line("Contact dabreegster@gmail.com to add your idea here!");
                txt.into_widget(ctx).centered_horiz()
            },
            Widget::custom_row(tab_buttons)
                .flex_wrap(ctx, Percent::int(80))
                .margin_above(60),
            Widget::col(current_tab_rows),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(Proposals { proposals, current }))
    }
}

impl SimpleState<App> for Proposals {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        _: &mut Panel,
    ) -> Transition {
        match x {
            "close" => Transition::Pop,
            "Try out this proposal" => launch(
                ctx,
                app,
                self.proposals[self.current.as_ref().unwrap()].clone(),
            ),
            "Read detailed write-up" => {
                open_browser(
                    self.proposals[self.current.as_ref().unwrap()]
                        .proposal_link
                        .clone()
                        .unwrap(),
                );
                Transition::Keep
            }
            x => Transition::Replace(Proposals::new_state(ctx, Some(x.to_string()))),
        }
    }
}

fn launch(ctx: &mut EventCtx, app: &App, edits: PermanentMapEdits) -> Transition {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !abstio::file_exists(edits.map_name.path()) {
            return map_gui::tools::prompt_to_download_missing_data(ctx, edits.map_name);
        }
    }

    Transition::Push(MapLoader::new_state(
        ctx,
        app,
        edits.map_name.clone(),
        Box::new(move |ctx, app| {
            // Apply edits before setting up the sandbox, for simplicity
            let maybe_err = ctx.loading_screen("apply edits", |ctx, timer| {
                match edits.into_edits(&app.primary.map) {
                    Ok(edits) => {
                        apply_map_edits(ctx, app, edits);
                        app.primary.map.recalculate_pathfinding_after_edits(timer);
                        None
                    }
                    Err(err) => Some(err),
                }
            });
            if let Some(err) = maybe_err {
                Transition::Replace(PopupMsg::new_state(
                    ctx,
                    "Can't load proposal",
                    vec![err.to_string()],
                ))
            } else {
                app.primary.layer = Some(Box::new(crate::layer::map::Static::edits(ctx, app)));
                Transition::Replace(SandboxMode::simple_new(
                    app,
                    GameplayMode::PlayScenario(
                        app.primary.map.get_name().clone(),
                        Scenario::default_scenario_for_map(app.primary.map.get_name()),
                        Vec::new(),
                    ),
                ))
            }
        }),
    ))
}

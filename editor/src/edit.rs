use crate::game::{GameState, Mode};
use crate::objects::DrawCtx;
use crate::plugins::{apply_map_edits, load_edits, PluginCtx};
use crate::render::{RenderOptions, Renderable, MIN_ZOOM_FOR_DETAIL};
use abstutil::Timer;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Wizard, WrappedWizard, GUI};
use map_model::Map;

pub enum EditMode {
    ViewingDiffs,
    Saving(Wizard),
    Loading(Wizard),
}

impl EditMode {
    pub fn event(state: &mut GameState, ctx: EventCtx) -> EventLoopMode {
        let edits = state.ui.state.primary.map.get_edits();

        // TODO Display info/hints on more lines.
        ctx.input.set_mode_with_prompt(
            "Map Edit Mode",
            format!("Map Edit Mode for {}", edits.describe()),
            &ctx.canvas,
        );
        // TODO Clicking this works, but the key doesn't
        if ctx.input.modal_action("quit") {
            // TODO Warn about unsaved edits
            state.mode = Mode::SplashScreen(Wizard::new(), None);
            return EventLoopMode::InputOnly;
        }

        match state.mode {
            Mode::Edit(EditMode::ViewingDiffs) => {
                // TODO Only if current edits are unsaved
                if ctx.input.modal_action("save edits") {
                    state.mode = Mode::Edit(EditMode::Saving(Wizard::new()));
                } else if ctx.input.modal_action("load different edits") {
                    state.mode = Mode::Edit(EditMode::Loading(Wizard::new()));
                }
            }
            Mode::Edit(EditMode::Saving(ref mut wizard)) => {
                if save_edits(
                    wizard.wrap(ctx.input, ctx.canvas),
                    &mut state.ui.state.primary.map,
                )
                .is_some()
                    || wizard.aborted()
                {
                    state.mode = Mode::Edit(EditMode::ViewingDiffs);
                }
            }
            Mode::Edit(EditMode::Loading(ref mut wizard)) => {
                if let Some(new_edits) = load_edits(
                    &state.ui.state.primary.map,
                    &mut wizard.wrap(ctx.input, ctx.canvas),
                    "Load which map edits?",
                ) {
                    apply_map_edits(
                        &mut PluginCtx {
                            primary: &mut state.ui.state.primary,
                            secondary: &mut None,
                            canvas: ctx.canvas,
                            cs: &mut state.ui.state.cs,
                            prerender: ctx.prerender,
                            input: ctx.input,
                            hints: &mut state.ui.hints,
                            recalculate_current_selection: &mut false,
                        },
                        new_edits,
                    );
                    state.mode = Mode::Edit(EditMode::ViewingDiffs);
                } else if wizard.aborted() {
                    state.mode = Mode::Edit(EditMode::ViewingDiffs);
                }
            }
            _ => unreachable!(),
        }

        ctx.canvas.handle_event(ctx.input);
        EventLoopMode::InputOnly
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        state.ui.draw(g);
        let ctx = DrawCtx {
            cs: &state.ui.state.cs,
            map: &state.ui.state.primary.map,
            draw_map: &state.ui.state.primary.draw_map,
            sim: &state.ui.state.primary.sim,
            hints: &state.ui.hints,
        };

        match state.mode {
            Mode::Edit(EditMode::ViewingDiffs) => {
                // TODO Similar to drawing areas with traffic or not -- would be convenient to just
                // supply a set of things to highlight and have something else take care of drawing
                // with detail or not.
                let zoomed = g.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL;

                // More generally we might want to show the diff between two edits, but for now,
                // just show diff relative to basemap.
                let edits = ctx.map.get_edits();
                for l in edits.lane_overrides.keys() {
                    if zoomed {
                        ctx.draw_map.get_l(*l).draw(
                            g,
                            RenderOptions {
                                color: Some(ctx.cs.get_def("map diffs", Color::RED)),
                                debug_mode: false,
                            },
                            &ctx,
                        );
                    } else {
                        g.draw_polygon(
                            ctx.cs.get("map diffs"),
                            &ctx.map.get_parent(*l).get_thick_polygon().unwrap(),
                        );
                    }
                }
                for i in edits
                    .stop_sign_overrides
                    .keys()
                    .chain(edits.traffic_signal_overrides.keys())
                {
                    ctx.draw_map.get_i(*i).draw(
                        g,
                        RenderOptions {
                            color: Some(ctx.cs.get("map diffs")),
                            debug_mode: false,
                        },
                        &ctx,
                    );
                }
            }
            Mode::Edit(EditMode::Saving(ref wizard))
            | Mode::Edit(EditMode::Loading(ref wizard)) => {
                // TODO Still draw the diffs, yo
                wizard.draw(g);
            }
            _ => unreachable!(),
        }
    }
}

fn save_edits(mut wizard: WrappedWizard, map: &mut Map) -> Option<()> {
    let rename = if map.get_edits().edits_name == "no_edits" {
        Some(wizard.input_string("Name these map edits")?)
    } else {
        None
    };

    // TODO Do it this weird way to avoid saving edits on every event. :P
    let save = "save edits";
    let cancel = "cancel";
    if wizard
        .choose_string("Overwrite edits?", vec![save, cancel])?
        .as_str()
        == save
    {
        if let Some(name) = rename {
            let mut edits = map.get_edits().clone();
            edits.edits_name = name;
            map.apply_edits(edits, &mut Timer::new("name map edits"));
        }
        map.get_edits().save();
    }
    Some(())
}

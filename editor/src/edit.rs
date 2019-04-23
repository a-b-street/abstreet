use crate::game::{GameState, Mode};
use crate::objects::DrawCtx;
use crate::render::{RenderOptions, Renderable, MIN_ZOOM_FOR_DETAIL};
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Wizard, GUI};

pub enum EditMode {
    ViewingDiffs,
    // loading others, saving, road editor, intersection editors, etc
}

impl EditMode {
    pub fn event(state: &mut GameState, ctx: EventCtx) -> EventLoopMode {
        // TODO modal with some info

        match state.mode {
            Mode::Edit(EditMode::ViewingDiffs) => {}
            _ => unreachable!(),
        }

        let (event_mode, pause) = state.ui.new_event(ctx);
        if pause {
            state.mode = Mode::SplashScreen(Wizard::new());
        }
        event_mode
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
            _ => unreachable!(),
        }
    }
}

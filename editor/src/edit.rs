use crate::game::{GameState, Mode};
use crate::objects::{DrawCtx, ID};
use crate::plugins::{apply_map_edits, load_edits, PluginCtx};
use crate::render::{RenderOptions, Renderable, MIN_ZOOM_FOR_DETAIL};
use abstutil::Timer;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Wizard, WrappedWizard, GUI};
use map_model::{Lane, LaneType, Map, Road};

pub enum EditMode {
    ViewingDiffs,
    Saving(Wizard),
    Loading(Wizard),
}

impl EditMode {
    pub fn event(state: &mut GameState, mut ctx: EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);

        // TODO Display info/hints on more lines.
        ctx.input.set_mode_with_prompt(
            "Map Edit Mode",
            format!(
                "Map Edit Mode for {}",
                state.ui.state.primary.map.get_edits().describe()
            ),
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

                // TODO Reset when transitioning in/out of this state? Or maybe we just don't draw
                // the effects of it. Or eventually, the Option<ID> itself will live in here
                // directly.
                // TODO Only mouseover lanes and intersections?
                state.ui.handle_mouseover(&mut ctx);

                if let Some(ID::Lane(id)) = state.ui.state.primary.current_selection {
                    let lane = state.ui.state.primary.map.get_l(id);
                    let road = state.ui.state.primary.map.get_r(lane.parent);

                    if lane.lane_type != LaneType::Sidewalk {
                        if let Some(new_type) = next_valid_type(road, lane) {
                            if ctx
                                .input
                                .contextual_action(Key::Space, &format!("toggle to {:?}", new_type))
                            {
                                let mut new_edits = state.ui.state.primary.map.get_edits().clone();
                                new_edits.lane_overrides.insert(lane.id, new_type);
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
                            }
                        }
                    }
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

// For lane editing

fn next_valid_type(r: &Road, l: &Lane) -> Option<LaneType> {
    let mut new_type = next_type(l.lane_type);
    while new_type != l.lane_type {
        if can_change_lane_type(r, l, new_type) {
            return Some(new_type);
        }
        new_type = next_type(new_type);
    }
    None
}

fn next_type(lt: LaneType) -> LaneType {
    match lt {
        LaneType::Driving => LaneType::Parking,
        LaneType::Parking => LaneType::Biking,
        LaneType::Biking => LaneType::Bus,
        LaneType::Bus => LaneType::Driving,

        LaneType::Sidewalk => unreachable!(),
    }
}

fn can_change_lane_type(r: &Road, l: &Lane, lt: LaneType) -> bool {
    let (fwds, idx) = r.dir_and_offset(l.id);

    // Only one parking lane per side.
    if lt == LaneType::Parking {
        let has_parking = if fwds {
            r.get_lane_types().0
        } else {
            r.get_lane_types().1
        }
        .contains(&LaneType::Parking);
        if has_parking {
            return false;
        }
    }

    // Two adjacent bike lanes is unnecessary.
    if lt == LaneType::Biking {
        let types = if fwds {
            r.get_lane_types().0
        } else {
            r.get_lane_types().1
        };
        if (idx != 0 && types[idx - 1] == LaneType::Biking)
            || types.get(idx + 1) == Some(&LaneType::Biking)
        {
            return false;
        }
    }

    true
}

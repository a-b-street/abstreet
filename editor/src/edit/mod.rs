mod traffic_signals;

use crate::game::{GameState, Mode};
use crate::objects::{DrawCtx, ID};
use crate::plugins::load_edits;
use crate::render::{DrawLane, DrawMap, DrawTurn, RenderOptions, Renderable, MIN_ZOOM_FOR_DETAIL};
use crate::state::UIState;
use abstutil::Timer;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, Wizard, WrappedWizard};
use map_model::{
    IntersectionID, Lane, LaneID, LaneType, Map, MapEdits, Road, TurnID, TurnPriority,
};
use std::collections::{BTreeSet, HashMap};

pub enum EditMode {
    ViewingDiffs,
    Saving(Wizard),
    Loading(Wizard),
    EditingStopSign(IntersectionID),
    EditingTrafficSignal(traffic_signals::TrafficSignalEditor),
}

impl EditMode {
    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);

        // Common functionality
        let mut txt = Text::new();
        txt.add_styled_line("Map Edit Mode".to_string(), None, Some(Color::BLUE), None);
        txt.add_line(state.ui.state.primary.map.get_edits().edits_name.clone());
        txt.add_line(state.ui.state.primary.map.get_edits().describe());
        txt.add_line("Right-click a lane or intersection to start editing".to_string());
        match state.mode {
            Mode::Edit(EditMode::ViewingDiffs)
            | Mode::Edit(EditMode::Saving(_))
            | Mode::Edit(EditMode::Loading(_)) => {
                // TODO Display info/hints on more lines.
                ctx.input
                    .set_mode_with_new_prompt("Map Edit Mode", txt, ctx.canvas);
                // TODO Clicking this works, but the key doesn't
                if ctx.input.modal_action("quit") {
                    // TODO Warn about unsaved edits
                    state.mode = Mode::SplashScreen(Wizard::new(), None);
                    return EventLoopMode::InputOnly;
                }
            }
            _ => {}
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
                state.ui.state.primary.current_selection =
                    state
                        .ui
                        .handle_mouseover(ctx, None, &state.ui.state.primary.sim);

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
                                apply_map_edits(&mut state.ui.state, ctx, new_edits);
                            }
                        }
                    }
                }
                if let Some(ID::Intersection(id)) = state.ui.state.primary.current_selection {
                    if state.ui.state.primary.map.maybe_get_stop_sign(id).is_some()
                        && ctx
                            .input
                            .contextual_action(Key::E, &format!("edit stop signs for {}", id))
                    {
                        state.mode = Mode::Edit(EditMode::EditingStopSign(id));
                    }
                    if state
                        .ui
                        .state
                        .primary
                        .map
                        .maybe_get_traffic_signal(id)
                        .is_some()
                        && ctx
                            .input
                            .contextual_action(Key::E, &format!("edit traffic signal for {}", id))
                    {
                        state.mode = Mode::Edit(EditMode::EditingTrafficSignal(
                            traffic_signals::TrafficSignalEditor::new(id, ctx, &mut state.ui),
                        ));
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
                    apply_map_edits(&mut state.ui.state, ctx, new_edits);
                    state.mode = Mode::Edit(EditMode::ViewingDiffs);
                } else if wizard.aborted() {
                    state.mode = Mode::Edit(EditMode::ViewingDiffs);
                }
            }
            Mode::Edit(EditMode::EditingStopSign(i)) => {
                state.ui.state.primary.current_selection =
                    state
                        .ui
                        .handle_mouseover(ctx, Some(i), &state.ui.state.primary.sim);

                ctx.input.set_mode_with_prompt(
                    "Stop Sign Editor",
                    format!("Stop Sign Editor for {}", i),
                    &ctx.canvas,
                );

                if let Some(ID::Turn(t)) = state.ui.state.primary.current_selection {
                    let mut sign = state.ui.state.primary.map.get_stop_sign(i).clone();
                    let next_priority = match sign.get_priority(t) {
                        TurnPriority::Banned => TurnPriority::Stop,
                        TurnPriority::Stop => TurnPriority::Yield,
                        TurnPriority::Yield => {
                            if sign.could_be_priority_turn(t, &state.ui.state.primary.map) {
                                TurnPriority::Priority
                            } else {
                                TurnPriority::Banned
                            }
                        }
                        TurnPriority::Priority => TurnPriority::Banned,
                    };
                    if ctx
                        .input
                        .contextual_action(Key::Space, &format!("toggle to {:?}", next_priority))
                    {
                        sign.turns.insert(t, next_priority);
                        let mut new_edits = state.ui.state.primary.map.get_edits().clone();
                        new_edits.stop_sign_overrides.insert(i, sign);
                        apply_map_edits(&mut state.ui.state, ctx, new_edits);
                    }
                } else if ctx.input.modal_action("quit") {
                    state.mode = Mode::Edit(EditMode::ViewingDiffs);
                } else if ctx.input.modal_action("reset to default") {
                    let mut new_edits = state.ui.state.primary.map.get_edits().clone();
                    new_edits.stop_sign_overrides.remove(&i);
                    apply_map_edits(&mut state.ui.state, ctx, new_edits);
                }
            }
            Mode::Edit(EditMode::EditingTrafficSignal(ref mut editor)) => {
                if editor.event(ctx, &mut state.ui) {
                    state.mode = Mode::Edit(EditMode::ViewingDiffs);
                }
            }
            _ => unreachable!(),
        }

        EventLoopMode::InputOnly
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        let mut override_color: HashMap<ID, Color> = HashMap::new();

        let ctx = DrawCtx {
            cs: &state.ui.state.cs,
            map: &state.ui.state.primary.map,
            draw_map: &state.ui.state.primary.draw_map,
            sim: &state.ui.state.primary.sim,
            hints: &state.ui.hints,
        };

        match state.mode {
            Mode::Edit(EditMode::ViewingDiffs) => {
                state
                    .ui
                    .new_draw(g, None, override_color, &state.ui.state.primary.sim);

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
                state
                    .ui
                    .new_draw(g, None, override_color, &state.ui.state.primary.sim);

                // TODO Still draw the diffs, yo
                wizard.draw(g);
            }
            Mode::Edit(EditMode::EditingStopSign(i)) => {
                let sign = state.ui.state.primary.map.get_stop_sign(i);
                for t in &state.ui.state.primary.map.get_i(i).turns {
                    override_color.insert(
                        ID::Turn(*t),
                        match sign.get_priority(*t) {
                            TurnPriority::Priority => state
                                .ui
                                .state
                                .cs
                                .get_def("priority stop sign turn", Color::GREEN),
                            TurnPriority::Yield => state
                                .ui
                                .state
                                .cs
                                .get_def("yield stop sign turn", Color::YELLOW),
                            TurnPriority::Stop => {
                                state.ui.state.cs.get_def("stop turn", Color::RED)
                            }
                            TurnPriority::Banned => {
                                state.ui.state.cs.get_def("banned turn", Color::BLACK)
                            }
                        },
                    );
                }
                state
                    .ui
                    .new_draw(g, Some(i), override_color, &state.ui.state.primary.sim);
            }
            Mode::Edit(EditMode::EditingTrafficSignal(ref editor)) => {
                editor.draw(g, state);
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

pub fn apply_map_edits(state: &mut UIState, ctx: &mut EventCtx, edits: MapEdits) {
    let mut timer = Timer::new("apply map edits");
    state.primary.current_flags.sim_flags.edits_name = edits.edits_name.clone();
    let (lanes_changed, turns_deleted, turns_added) =
        state.primary.map.apply_edits(edits, &mut timer);

    for l in lanes_changed {
        state.primary.draw_map.lanes[l.0] = DrawLane::new(
            state.primary.map.get_l(l),
            &state.primary.map,
            !state.primary.current_flags.dont_draw_lane_markings,
            &state.cs,
            ctx.prerender,
            &mut timer,
        );
    }
    let mut lanes_of_modified_turns: BTreeSet<LaneID> = BTreeSet::new();
    for t in turns_deleted {
        state.primary.draw_map.turns.remove(&t);
        lanes_of_modified_turns.insert(t.src);
    }
    for t in &turns_added {
        lanes_of_modified_turns.insert(t.src);
    }

    let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
    for l in lanes_of_modified_turns {
        DrawMap::compute_turn_to_lane_offset(
            &mut turn_to_lane_offset,
            state.primary.map.get_l(l),
            &state.primary.map,
        );
    }
    for t in turns_added {
        state.primary.draw_map.turns.insert(
            t,
            DrawTurn::new(
                &state.primary.map,
                state.primary.map.get_t(t),
                turn_to_lane_offset[&t],
            ),
        );
    }
}

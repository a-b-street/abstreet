mod traffic_signals;

use crate::common::CommonState;
use crate::game::{GameState, Mode};
use crate::helpers::ID;
use crate::render::{
    DrawCtx, DrawLane, DrawMap, DrawOptions, DrawTurn, Renderable, MIN_ZOOM_FOR_DETAIL,
};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, Wizard, WrappedWizard};
use map_model::{
    IntersectionID, Lane, LaneID, LaneType, Map, MapEdits, Road, TurnID, TurnPriority, TurnType,
};
use std::collections::{BTreeSet, HashMap};

pub enum EditMode {
    ViewingDiffs(CommonState),
    Saving(Wizard),
    Loading(Wizard),
    EditingStopSign(IntersectionID),
    EditingTrafficSignal(traffic_signals::TrafficSignalEditor),
}

impl EditMode {
    pub fn new() -> EditMode {
        EditMode::ViewingDiffs(CommonState::new())
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);

        // Common functionality
        let mut txt = Text::prompt("Map Edit Mode");
        txt.add_line(state.ui.primary.map.get_edits().edits_name.clone());
        txt.add_line(state.ui.primary.map.get_edits().describe());
        txt.add_line("Right-click a lane or intersection to start editing".to_string());
        match state.mode {
            Mode::Edit(EditMode::ViewingDiffs(_))
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
            Mode::Edit(EditMode::ViewingDiffs(ref mut common)) => {
                // TODO Reset when transitioning in/out of this state? Or maybe we just don't draw
                // the effects of it. Or eventually, the Option<ID> itself will live in here
                // directly.
                // TODO Only mouseover lanes and intersections?
                state.ui.primary.current_selection = state.ui.handle_mouseover(
                    ctx,
                    None,
                    &state.ui.primary.sim,
                    &ShowEverything::new(),
                    false,
                );
                if let Some(evmode) = common.event(ctx, &state.ui) {
                    return evmode;
                }

                // TODO Only if current edits are unsaved
                if ctx.input.modal_action("save edits") {
                    state.mode = Mode::Edit(EditMode::Saving(Wizard::new()));
                    return EventLoopMode::InputOnly;
                } else if ctx.input.modal_action("load different edits") {
                    state.mode = Mode::Edit(EditMode::Loading(Wizard::new()));
                    return EventLoopMode::InputOnly;
                }

                if let Some(ID::Lane(id)) = state.ui.primary.current_selection {
                    let lane = state.ui.primary.map.get_l(id);
                    let road = state.ui.primary.map.get_r(lane.parent);

                    if lane.lane_type != LaneType::Sidewalk {
                        if let Some(new_type) = next_valid_type(road, lane) {
                            if ctx
                                .input
                                .contextual_action(Key::Space, &format!("toggle to {:?}", new_type))
                            {
                                let mut new_edits = state.ui.primary.map.get_edits().clone();
                                new_edits.lane_overrides.insert(lane.id, new_type);
                                apply_map_edits(&mut state.ui, ctx, new_edits);
                            }
                        }
                    }
                }
                if let Some(ID::Intersection(id)) = state.ui.primary.current_selection {
                    if state.ui.primary.map.maybe_get_stop_sign(id).is_some()
                        && ctx
                            .input
                            .contextual_action(Key::E, &format!("edit stop signs for {}", id))
                    {
                        state.mode = Mode::Edit(EditMode::EditingStopSign(id));
                    }
                    if state.ui.primary.map.maybe_get_traffic_signal(id).is_some()
                        && ctx
                            .input
                            .contextual_action(Key::E, &format!("edit traffic signal for {}", id))
                    {
                        state.mode = Mode::Edit(EditMode::EditingTrafficSignal(
                            traffic_signals::TrafficSignalEditor::new(id, ctx),
                        ));
                    }
                }
            }
            Mode::Edit(EditMode::Saving(ref mut wizard)) => {
                if save_edits(
                    wizard.wrap(ctx.input, ctx.canvas),
                    &mut state.ui.primary.map,
                )
                .is_some()
                    || wizard.aborted()
                {
                    state.mode = Mode::Edit(EditMode::new());
                }
            }
            Mode::Edit(EditMode::Loading(ref mut wizard)) => {
                if let Some(new_edits) = load_edits(
                    &state.ui.primary.map,
                    &mut wizard.wrap(ctx.input, ctx.canvas),
                    "Load which map edits?",
                ) {
                    apply_map_edits(&mut state.ui, ctx, new_edits);
                    state.mode = Mode::Edit(EditMode::new());
                } else if wizard.aborted() {
                    state.mode = Mode::Edit(EditMode::new());
                }
            }
            Mode::Edit(EditMode::EditingStopSign(i)) => {
                state.ui.primary.current_selection = state.ui.handle_mouseover(
                    ctx,
                    Some(i),
                    &state.ui.primary.sim,
                    &ShowEverything::new(),
                    false,
                );

                ctx.input.set_mode_with_prompt(
                    "Stop Sign Editor",
                    format!("Stop Sign Editor for {}", i),
                    &ctx.canvas,
                );

                if let Some(ID::Turn(t)) = state.ui.primary.current_selection {
                    let mut sign = state.ui.primary.map.get_stop_sign(i).clone();
                    let next_priority = match sign.get_priority(t) {
                        TurnPriority::Banned => TurnPriority::Stop,
                        TurnPriority::Stop => TurnPriority::Yield,
                        TurnPriority::Yield => {
                            if sign.could_be_priority_turn(t, &state.ui.primary.map) {
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
                        let mut new_edits = state.ui.primary.map.get_edits().clone();
                        new_edits.stop_sign_overrides.insert(i, sign);
                        apply_map_edits(&mut state.ui, ctx, new_edits);
                    }
                } else if ctx.input.modal_action("quit") {
                    state.mode = Mode::Edit(EditMode::new());
                } else if ctx.input.modal_action("reset to default") {
                    let mut new_edits = state.ui.primary.map.get_edits().clone();
                    new_edits.stop_sign_overrides.remove(&i);
                    apply_map_edits(&mut state.ui, ctx, new_edits);
                }
            }
            Mode::Edit(EditMode::EditingTrafficSignal(ref mut editor)) => {
                if editor.event(ctx, &mut state.ui) {
                    state.mode = Mode::Edit(EditMode::new());
                }
            }
            _ => unreachable!(),
        }

        EventLoopMode::InputOnly
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        match state.mode {
            Mode::Edit(EditMode::ViewingDiffs(ref common)) => {
                state.ui.draw(
                    g,
                    common.draw_options(&state.ui),
                    &state.ui.primary.sim,
                    &ShowEverything::new(),
                );
                common.draw(g, &state.ui);

                // TODO Similar to drawing areas with traffic or not -- would be convenient to just
                // supply a set of things to highlight and have something else take care of drawing
                // with detail or not.
                let zoomed = g.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL;
                let color = if zoomed {
                    state
                        .ui
                        .cs
                        .get_def("zoomed map diffs", Color::RED.alpha(0.5))
                } else {
                    state.ui.cs.get_def("unzoomed map diffs", Color::RED)
                };

                let ctx = DrawCtx {
                    cs: &state.ui.cs,
                    map: &state.ui.primary.map,
                    draw_map: &state.ui.primary.draw_map,
                    sim: &state.ui.primary.sim,
                };
                let mut opts = DrawOptions::new();

                // More generally we might want to show the diff between two edits, but for now,
                // just show diff relative to basemap.
                let edits = state.ui.primary.map.get_edits();
                for l in edits.lane_overrides.keys() {
                    if zoomed {
                        opts.override_colors.insert(ID::Lane(*l), color);
                        state.ui.primary.draw_map.get_l(*l).draw(g, &opts, &ctx);
                    } else {
                        g.draw_polygon(
                            color,
                            &state
                                .ui
                                .primary
                                .map
                                .get_parent(*l)
                                .get_thick_polygon()
                                .unwrap(),
                        );
                    }
                }
                for i in edits
                    .stop_sign_overrides
                    .keys()
                    .chain(edits.traffic_signal_overrides.keys())
                {
                    opts.override_colors.insert(ID::Intersection(*i), color);
                    state.ui.primary.draw_map.get_i(*i).draw(g, &opts, &ctx);
                }
            }
            Mode::Edit(EditMode::Saving(ref wizard))
            | Mode::Edit(EditMode::Loading(ref wizard)) => {
                state.ui.draw(
                    g,
                    DrawOptions::new(),
                    &state.ui.primary.sim,
                    &ShowEverything::new(),
                );

                // TODO Still draw the diffs, yo
                wizard.draw(g);
            }
            Mode::Edit(EditMode::EditingStopSign(i)) => {
                let mut opts = DrawOptions::new();
                opts.show_turn_icons_for = Some(i);
                let sign = state.ui.primary.map.get_stop_sign(i);
                for t in &state.ui.primary.map.get_i(i).turns {
                    opts.override_colors.insert(
                        ID::Turn(*t),
                        match sign.get_priority(*t) {
                            TurnPriority::Priority => {
                                state.ui.cs.get_def("priority stop sign turn", Color::GREEN)
                            }
                            TurnPriority::Yield => {
                                state.ui.cs.get_def("yield stop sign turn", Color::YELLOW)
                            }
                            TurnPriority::Stop => state.ui.cs.get_def("stop turn", Color::RED),
                            TurnPriority::Banned => {
                                state.ui.cs.get_def("banned turn", Color::BLACK)
                            }
                        },
                    );
                }
                state
                    .ui
                    .draw(g, opts, &state.ui.primary.sim, &ShowEverything::new());

                if let Some(ID::Turn(id)) = state.ui.primary.current_selection {
                    DrawTurn::draw_dashed(
                        state.ui.primary.map.get_t(id),
                        g,
                        state.ui.cs.get_def("selected turn", Color::RED),
                    );
                }
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

pub fn apply_map_edits(ui: &mut UI, ctx: &mut EventCtx, edits: MapEdits) {
    let mut timer = Timer::new("apply map edits");
    ui.primary.current_flags.sim_flags.edits_name = edits.edits_name.clone();
    let (lanes_changed, turns_deleted, turns_added) = ui.primary.map.apply_edits(edits, &mut timer);

    for l in lanes_changed {
        ui.primary.draw_map.lanes[l.0] = DrawLane::new(
            ui.primary.map.get_l(l),
            &ui.primary.map,
            !ui.primary.current_flags.dont_draw_lane_markings,
            &ui.cs,
            ctx.prerender,
            &mut timer,
        );
    }
    let mut lanes_of_modified_turns: BTreeSet<LaneID> = BTreeSet::new();
    for t in turns_deleted {
        ui.primary.draw_map.turns.remove(&t);
        lanes_of_modified_turns.insert(t.src);
    }
    for t in &turns_added {
        lanes_of_modified_turns.insert(t.src);
    }

    let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
    for l in lanes_of_modified_turns {
        DrawMap::compute_turn_to_lane_offset(
            &mut turn_to_lane_offset,
            ui.primary.map.get_l(l),
            &ui.primary.map,
        );
    }
    for t in turns_added {
        let turn = ui.primary.map.get_t(t);
        if turn.turn_type != TurnType::SharedSidewalkCorner {
            ui.primary.draw_map.turns.insert(
                t,
                DrawTurn::new(&ui.primary.map, turn, turn_to_lane_offset[&t]),
            );
        }
    }
}

fn load_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<MapEdits> {
    // TODO Exclude current?
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<MapEdits>(
            query,
            Box::new(move || {
                let mut list = abstutil::load_all_objects("edits", &map_name);
                list.push(("no_edits".to_string(), MapEdits::new(map_name.clone())));
                list
            }),
        )
        .map(|(_, e)| e)
}

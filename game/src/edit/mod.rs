mod stop_signs;
mod traffic_signals;

use crate::common::CommonState;
use crate::debug::DebugMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::{ColorScheme, ID};
use crate::render::{
    DrawCtx, DrawIntersection, DrawLane, DrawMap, DrawOptions, DrawTurn, Renderable,
    MIN_ZOOM_FOR_DETAIL,
};
use crate::sandbox::SandboxMode;
use crate::ui::{PerMapUI, ShowEverything, UI};
use abstutil::Timer;
use ezgui::{hotkey, lctrl, Choice, Color, EventCtx, GfxCtx, Key, Line, ModalMenu, Text, Wizard};
use map_model::{
    IntersectionID, Lane, LaneID, LaneType, Map, MapEdits, Road, RoadID, TurnID, TurnType,
};
use std::collections::{BTreeSet, HashMap};

pub struct EditMode {
    common: CommonState,
    menu: ModalMenu,
}

impl EditMode {
    pub fn new(ctx: &EventCtx, ui: &mut UI) -> EditMode {
        // TODO Warn first?
        ui.primary.reset_sim();

        EditMode {
            common: CommonState::new(),
            menu: ModalMenu::new(
                "Map Edit Mode",
                vec![
                    vec![
                        (hotkey(Key::S), "save edits"),
                        (hotkey(Key::L), "load different edits"),
                    ],
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (lctrl(Key::S), "sandbox mode"),
                        (lctrl(Key::D), "debug mode"),
                        (hotkey(Key::J), "warp"),
                        (hotkey(Key::K), "navigate"),
                        (hotkey(Key::SingleQuote), "shortcuts"),
                        (hotkey(Key::F1), "take a screenshot"),
                    ],
                ],
                ctx,
            ),
        }
    }
}

impl State for EditMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // The .clone() is probably not that expensive, and it makes later code a bit
        // easier to read. :)
        let orig_edits = ui.primary.map.get_edits().clone();
        let mut txt = Text::prompt("Map Edit Mode");
        {
            txt.add(Line(&orig_edits.edits_name));
            txt.add(Line(format!("{} lanes", orig_edits.lane_overrides.len())));
            txt.add(Line(format!(
                "{} stop signs ",
                orig_edits.stop_sign_overrides.len()
            )));
            txt.add(Line(format!(
                "{} traffic signals",
                orig_edits.traffic_signal_overrides.len()
            )));
            txt.add(Line("Right-click a lane or intersection to start editing"));
        }
        self.menu.handle_event(ctx, Some(txt));

        ctx.canvas.handle_event(ctx.input);

        // TODO Reset when transitioning in/out of this state? Or maybe we just don't draw
        // the effects of it. Or eventually, the Option<ID> itself will live in here
        // directly.
        // TODO Only mouseover lanes and intersections?
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        if let Some(t) = self.common.event(ctx, ui, &mut self.menu) {
            return t;
        }

        if self.menu.action("quit") {
            return Transition::Pop;
        }
        if self.menu.action("sandbox mode") {
            return Transition::Replace(Box::new(SandboxMode::new(ctx)));
        }
        if self.menu.action("debug mode") {
            return Transition::Replace(Box::new(DebugMode::new(ctx, ui)));
        }

        // TODO Only if current edits are unsaved
        if self.menu.action("save edits") {
            return Transition::Push(WizardState::new(Box::new(save_edits)));
        } else if self.menu.action("load different edits") {
            return Transition::Push(WizardState::new(Box::new(load_edits)));
        }

        if let Some(ID::Lane(id)) = ui.primary.current_selection {
            // TODO Urgh, borrow checker.
            {
                let lane = ui.primary.map.get_l(id);
                let road = ui.primary.map.get_r(lane.parent);
                if lane.lane_type != LaneType::Sidewalk {
                    if let Some(new_type) = next_valid_type(road, lane, &ui.primary.map) {
                        if ctx
                            .input
                            .contextual_action(Key::Space, format!("toggle to {:?}", new_type))
                        {
                            let mut new_edits = orig_edits.clone();
                            new_edits.lane_overrides.insert(lane.id, new_type);
                            apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
                        }
                    }
                }
            }
            {
                let lane = ui.primary.map.get_l(id);
                let road = ui.primary.map.get_r(lane.parent);
                if lane.lane_type != LaneType::Sidewalk {
                    for (lt, name, key) in &[
                        (LaneType::Driving, "driving", Key::D),
                        (LaneType::Parking, "parking", Key::P),
                        (LaneType::Biking, "biking", Key::B),
                        (LaneType::Bus, "bus", Key::T),
                    ] {
                        if can_change_lane_type(road, lane, *lt, &ui.primary.map)
                            && ctx
                                .input
                                .contextual_action(*key, format!("change to {} lane", name))
                        {
                            let mut new_edits = orig_edits.clone();
                            new_edits.lane_overrides.insert(lane.id, *lt);
                            apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
                            break;
                        }
                    }
                }
            }

            if ctx
                .input
                .contextual_action(Key::U, "bulk edit lanes on this road")
            {
                return Transition::Push(make_bulk_edit_lanes(ui.primary.map.get_l(id).parent));
            } else if orig_edits.lane_overrides.contains_key(&id)
                && ctx.input.contextual_action(Key::R, "revert")
            {
                let mut new_edits = orig_edits.clone();
                new_edits.lane_overrides.remove(&id);
                apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
            }
        }
        if let Some(ID::Intersection(id)) = ui.primary.current_selection {
            if ui.primary.map.maybe_get_stop_sign(id).is_some() {
                if ctx
                    .input
                    .contextual_action(Key::E, format!("edit stop signs for {}", id))
                {
                    return Transition::Push(Box::new(stop_signs::StopSignEditor::new(
                        id, ctx, ui,
                    )));
                } else if orig_edits.stop_sign_overrides.contains_key(&id)
                    && ctx.input.contextual_action(Key::R, "revert")
                {
                    let mut new_edits = orig_edits.clone();
                    new_edits.stop_sign_overrides.remove(&id);
                    apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
                }
            }
            if ui.primary.map.maybe_get_traffic_signal(id).is_some() {
                if ctx
                    .input
                    .contextual_action(Key::E, format!("edit traffic signal for {}", id))
                {
                    return Transition::Push(Box::new(traffic_signals::TrafficSignalEditor::new(
                        id, ctx, ui,
                    )));
                } else if orig_edits.traffic_signal_overrides.contains_key(&id)
                    && ctx.input.contextual_action(Key::R, "revert")
                {
                    let mut new_edits = orig_edits.clone();
                    new_edits.traffic_signal_overrides.remove(&id);
                    apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
                }
            }
        }

        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        ui.draw(
            g,
            self.common.draw_options(ui),
            &ui.primary.sim,
            &ShowEverything::new(),
        );

        // More generally we might want to show the diff between two edits, but for now,
        // just show diff relative to basemap.
        let edits = ui.primary.map.get_edits();

        let ctx = DrawCtx {
            cs: &ui.cs,
            map: &ui.primary.map,
            draw_map: &ui.primary.draw_map,
            sim: &ui.primary.sim,
        };
        let mut opts = DrawOptions::new();

        // TODO Similar to drawing areas with traffic or not -- would be convenient to just
        // supply a set of things to highlight and have something else take care of drawing
        // with detail or not.
        if g.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL {
            for l in edits.lane_overrides.keys() {
                opts.override_colors.insert(ID::Lane(*l), Color::Hatching);
                ctx.draw_map.get_l(*l).draw(g, &opts, &ctx);
            }
            for i in edits
                .stop_sign_overrides
                .keys()
                .chain(edits.traffic_signal_overrides.keys())
            {
                opts.override_colors
                    .insert(ID::Intersection(*i), Color::Hatching);
                ctx.draw_map.get_i(*i).draw(g, &opts, &ctx);
            }

            // The hatching covers up the selection outline, so redraw it.
            match ui.primary.current_selection {
                Some(ID::Lane(l)) => {
                    g.draw_polygon(
                        ui.cs.get("selected"),
                        &ctx.draw_map.get_l(l).get_outline(&ctx.map),
                    );
                }
                Some(ID::Intersection(i)) => {
                    g.draw_polygon(
                        ui.cs.get("selected"),
                        &ctx.draw_map.get_i(i).get_outline(&ctx.map),
                    );
                }
                _ => {}
            }
        } else {
            let color = ui.cs.get_def("unzoomed map diffs", Color::RED);
            for l in edits.lane_overrides.keys() {
                g.draw_polygon(color, &ctx.map.get_parent(*l).get_thick_polygon().unwrap());
            }

            for i in edits
                .stop_sign_overrides
                .keys()
                .chain(edits.traffic_signal_overrides.keys())
            {
                opts.override_colors.insert(ID::Intersection(*i), color);
                ctx.draw_map.get_i(*i).draw(g, &opts, &ctx);
            }
        }

        self.common.draw(g, ui);
        self.menu.draw(g);
    }

    fn on_destroy(&mut self, _: &mut EventCtx, ui: &mut UI) {
        // TODO Warn about unsaved edits
        // TODO Maybe put a loading screen around these.
        ui.primary
            .map
            .recalculate_pathfinding_after_edits(&mut Timer::new("apply pending map edits"));
        // Parking state might've changed
        ui.primary.reset_sim();
    }
}

fn save_edits(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let map = &mut ui.primary.map;
    let mut wizard = wiz.wrap(ctx);

    let rename = if map.get_edits().edits_name == "no_edits" {
        Some(wizard.input_string("Name these map edits")?)
    } else {
        None
    };

    // TODO Do it this weird way to avoid saving edits on every event. :P
    let save = "save edits";
    let cancel = "cancel";
    if wizard
        .choose_string("Overwrite edits?", || vec![save, cancel])?
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
    Some(Transition::Pop)
}

fn load_edits(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let map = &mut ui.primary.map;
    let mut wizard = wiz.wrap(ctx);

    // TODO Exclude current
    let map_name = map.get_name().to_string();
    let (_, new_edits) = wizard.choose("Load which map edits?", || {
        let mut list = Choice::from(abstutil::load_all_objects("edits", &map_name));
        list.push(Choice::new("no_edits", MapEdits::new(map_name.clone())));
        list
    })?;
    apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
    Some(Transition::Pop)
}

// For lane editing

fn next_valid_type(r: &Road, l: &Lane, map: &Map) -> Option<LaneType> {
    let mut new_type = next_type(l.lane_type);
    while new_type != l.lane_type {
        if can_change_lane_type(r, l, new_type, map) {
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

fn can_change_lane_type(r: &Road, l: &Lane, new_lt: LaneType, map: &Map) -> bool {
    let (fwds, idx) = r.dir_and_offset(l.id);
    let mut proposed_lts = if fwds {
        r.get_lane_types().0
    } else {
        r.get_lane_types().1
    };
    proposed_lts[idx] = new_lt;

    // No-op change
    if l.lane_type == new_lt {
        return false;
    }

    // Only one parking lane per side.
    if proposed_lts
        .iter()
        .filter(|lt| **lt == LaneType::Parking)
        .count()
        > 1
    {
        return false;
    }

    // Two adjacent bike lanes is unnecessary.
    for pair in proposed_lts.windows(2) {
        if pair[0] == LaneType::Biking && pair[1] == LaneType::Biking {
            return false;
        }
    }

    // Don't let players orphan a bus stop.
    if !r.all_bus_stops(map).is_empty()
        && (new_lt == LaneType::Parking || new_lt == LaneType::Biking)
    {
        // Is this the last one?
        let mut other_bus_lane = false;
        for id in r.all_lanes() {
            if l.id != id {
                let other_lt = map.get_l(id).lane_type;
                if other_lt == LaneType::Driving || other_lt == LaneType::Bus {
                    other_bus_lane = true;
                    break;
                }
            }
        }
        if !other_bus_lane {
            return false;
        }
    }

    // A parking lane must have a driving lane on the same side of the road.
    if proposed_lts.contains(&LaneType::Parking) && !proposed_lts.contains(&LaneType::Driving) {
        return false;
    }

    true
}

pub fn apply_map_edits(
    bundle: &mut PerMapUI,
    cs: &ColorScheme,
    ctx: &mut EventCtx,
    edits: MapEdits,
) {
    let mut timer = Timer::new("apply map edits");

    let (lanes_changed, turns_deleted, turns_added) = bundle.map.apply_edits(edits, &mut timer);

    for l in lanes_changed {
        bundle.draw_map.lanes[l.0] = DrawLane::new(
            bundle.map.get_l(l),
            &bundle.map,
            !bundle.current_flags.dont_draw_lane_markings,
            cs,
            &mut timer,
        )
        .finish(ctx.prerender);
    }
    let mut modified_intersections: BTreeSet<IntersectionID> = BTreeSet::new();
    let mut lanes_of_modified_turns: BTreeSet<LaneID> = BTreeSet::new();
    for t in turns_deleted {
        bundle.draw_map.turns.remove(&t);
        lanes_of_modified_turns.insert(t.src);
        modified_intersections.insert(t.parent);
    }
    for t in &turns_added {
        lanes_of_modified_turns.insert(t.src);
        modified_intersections.insert(t.parent);
    }

    let mut turn_to_lane_offset: HashMap<TurnID, usize> = HashMap::new();
    for l in lanes_of_modified_turns {
        DrawMap::compute_turn_to_lane_offset(
            &mut turn_to_lane_offset,
            bundle.map.get_l(l),
            &bundle.map,
        );
    }
    for t in turns_added {
        let turn = bundle.map.get_t(t);
        if turn.turn_type != TurnType::SharedSidewalkCorner {
            bundle
                .draw_map
                .turns
                .insert(t, DrawTurn::new(&bundle.map, turn, turn_to_lane_offset[&t]));
        }
    }

    for i in modified_intersections {
        bundle.draw_map.intersections[i.0] = DrawIntersection::new(
            bundle.map.get_i(i),
            &bundle.map,
            cs,
            ctx.prerender,
            &mut timer,
        );
    }

    // Do this after fixing up all the state above.
    bundle.map.simplify_edits(&mut timer);
}

fn make_bulk_edit_lanes(road: RoadID) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let mut wizard = wiz.wrap(ctx);
        let (_, from) = wizard.choose("Change all lanes of type...", || {
            vec![
                Choice::new("driving", LaneType::Driving),
                Choice::new("parking", LaneType::Parking),
                Choice::new("biking", LaneType::Biking),
                Choice::new("bus", LaneType::Bus),
            ]
        })?;
        let (_, to) = wizard.choose("Change to all lanes of type...", || {
            vec![
                Choice::new("driving", LaneType::Driving),
                Choice::new("parking", LaneType::Parking),
                Choice::new("biking", LaneType::Biking),
                Choice::new("bus", LaneType::Bus),
            ]
            .into_iter()
            .filter(|c| c.data != from)
            .collect()
        })?;

        // Do the dirty deed. Match by road name; OSM way ID changes a fair bit.
        let map = &ui.primary.map;
        let road_name = map.get_r(road).get_name();
        let mut edits = map.get_edits().clone();
        let mut cnt = 0;
        for l in map.all_lanes() {
            if l.lane_type != from {
                continue;
            }
            let parent = map.get_parent(l.id);
            if parent.get_name() != road_name {
                continue;
            }
            // TODO This looks at the original state of the map, not with all the edits applied so far!
            if can_change_lane_type(parent, l, to, map) {
                edits.lane_overrides.insert(l.id, to);
                cnt += 1;
            }
        }
        // TODO pop this up. warn about road names changing and being weird. :)
        println!(
            "Changed {} {:?} lanes to {:?} lanes on {}",
            cnt, from, to, road_name
        );
        apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
        Some(Transition::Pop)
    }))
}

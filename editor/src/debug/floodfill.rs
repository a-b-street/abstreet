use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::render::{DrawOptions, MIN_ZOOM_FOR_DETAIL};
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, Color, EventCtx, GfxCtx, Key, ModalMenu};
use map_model::{LaneID, LaneType, Map};
use std::collections::{HashMap, HashSet};

pub struct Floodfiller {
    menu: ModalMenu,
    override_colors: HashMap<ID, Color>,
}

impl Floodfiller {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<Box<State>> {
        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            let lt = ui.primary.map.get_l(l).lane_type;
            if lt != LaneType::Parking
                && ctx
                    .input
                    .contextual_action(Key::F, "floodfill from this lane")
            {
                let reachable = find_reachable_from(l, &ui.primary.map);
                let mut override_colors = HashMap::new();
                for lane in ui.primary.map.all_lanes() {
                    // TODO Not quite right when starting from bus and bike lanes
                    if lane.lane_type != lt {
                        continue;
                    }
                    let color = if reachable.contains(&lane.id) {
                        ui.cs.get_def("reachable lane", Color::GREEN)
                    } else {
                        ui.cs.get_def("unreachable lane", Color::RED)
                    };
                    override_colors.insert(ID::Lane(lane.id), color);
                }

                return Some(Box::new(Floodfiller {
                    menu: ModalMenu::new(
                        format!("Floodfiller from {}", l).as_str(),
                        vec![vec![(hotkey(Key::Escape), "quit")]],
                        ctx,
                    ),
                    override_colors,
                }));
            }
        }
        None
    }
}

impl State for Floodfiller {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        // TODO How many lanes NOT reachable? Show in menu
        self.menu.handle_event(ctx, None);
        if self.menu.action("quit") {
            return Transition::Pop;
        }

        Transition::Keep
    }

    // TODO Want this, but DebugMode acts a base. Unclear what plugins are useful to stack there,
    // actually...
    /*fn draw_default_ui(&self) -> bool {
        false
    }*/

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = DrawOptions::new();
        opts.override_colors = self.override_colors.clone();
        ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());

        // TODO No really, refactor this logic.
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            // TODO Dedupe roads... or even better, color mixes differently, or make the negative
            // case win.
            for (id, color) in &self.override_colors {
                let l = if let ID::Lane(l) = id {
                    *l
                } else {
                    unreachable!()
                };
                g.draw_polygon(
                    *color,
                    &ui.primary.map.get_parent(l).get_thick_polygon().unwrap(),
                );
            }
        }
    }
}

fn find_reachable_from(start: LaneID, map: &Map) -> HashSet<LaneID> {
    let mut visited = HashSet::new();
    let mut queue = vec![start];
    while !queue.is_empty() {
        let current = queue.pop().unwrap();
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current);
        for turn in map.get_turns_from_lane(current) {
            if map.is_turn_allowed(turn.id) && !visited.contains(&turn.id.dst) {
                queue.push(turn.id.dst);
            }
        }
    }
    visited
}

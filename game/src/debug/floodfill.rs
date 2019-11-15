use crate::common::{RoadColorer, RoadColorerBuilder};
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{hotkey, Color, EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use map_model::{connectivity, LaneID, Map, PathConstraints};
use std::collections::HashSet;

pub struct Floodfiller {
    menu: ModalMenu,
    colorer: RoadColorer,
}

impl Floodfiller {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<Box<dyn State>> {
        let map = &ui.primary.map;
        let (reachable_lanes, unreachable_lanes, title) =
            if let Some(ID::Lane(l)) = ui.primary.current_selection {
                let lt = map.get_l(l).lane_type;
                if !lt.supports_any_movement() {
                    return None;
                }
                if ctx
                    .input
                    .contextual_action(Key::F, "floodfill from this lane")
                {
                    find_reachable_from(l, map)
                } else if ctx
                    .input
                    .contextual_action(Key::S, "show strongly-connected components")
                {
                    let constraints = PathConstraints::from_lt(lt);
                    let (good, bad) = connectivity::find_scc(map, constraints);
                    (
                        good,
                        bad,
                        format!("strongly-connected components for {:?}", constraints),
                    )
                } else {
                    return None;
                }
            } else {
                return None;
            };

        let reachable_color = ui.cs.get_def("reachable lane", Color::GREEN);
        let unreachable_color = ui.cs.get_def("unreachable lane", Color::RED);

        let mut colorer = RoadColorerBuilder::new(
            Text::prompt("lane connectivity"),
            vec![
                ("unreachable", unreachable_color),
                ("reachable", reachable_color),
            ],
        );
        for l in reachable_lanes {
            colorer.add(l, reachable_color, map);
        }
        let num_unreachable = unreachable_lanes.len();
        for l in unreachable_lanes {
            colorer.add(l, unreachable_color, map);
            println!("{} is unreachable", l);
        }

        let mut menu = ModalMenu::new(title, vec![(hotkey(Key::Escape), "quit")], ctx);
        menu.set_info(
            ctx,
            Text::from(Line(format!("{} unreachable lanes", num_unreachable))),
        );

        Some(Box::new(Floodfiller {
            menu,
            colorer: colorer.build(ctx, map),
        }))
    }
}

impl State for Floodfiller {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        self.menu.event(ctx);
        if self.menu.action("quit") {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.colorer.draw(g, ui);
        self.menu.draw(g);
    }
}

// (reachable, unreachable, a title)
fn find_reachable_from(start: LaneID, map: &Map) -> (HashSet<LaneID>, HashSet<LaneID>, String) {
    let constraints = PathConstraints::from_lt(map.get_l(start).lane_type);

    let mut visited = HashSet::new();
    let mut queue = vec![start];
    while !queue.is_empty() {
        let current = queue.pop().unwrap();
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current);
        for turn in map.get_turns_for(current, constraints) {
            if !visited.contains(&turn.id.dst) {
                queue.push(turn.id.dst);
            }
        }
    }

    let mut unreached = HashSet::new();
    for l in map.all_lanes() {
        if constraints.can_use(l, map) && !visited.contains(&l.id) {
            unreached.insert(l.id);
        }
    }

    (
        visited,
        unreached,
        format!("Floodfiller for {:?} from {}", constraints, start),
    )
}

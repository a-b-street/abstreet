use crate::common::Colorer;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::managed::WrappedComposite;
use crate::ui::UI;
use ezgui::{Color, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Text};
use map_model::{connectivity, LaneID, Map, PathConstraints};
use std::collections::HashSet;

pub struct Floodfiller {
    composite: Composite,
    colorer: Colorer,
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
                if ui.per_obj.action(ctx, Key::F, "floodfill from this lane") {
                    find_reachable_from(l, map)
                } else if ui
                    .per_obj
                    .action(ctx, Key::S, "show strongly-connected components")
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

        let mut colorer = Colorer::new(
            Text::from(Line("lane connectivity")),
            vec![
                ("unreachable", unreachable_color),
                ("reachable", reachable_color),
            ],
        );
        for l in reachable_lanes {
            colorer.add_l(l, reachable_color, map);
        }
        let num_unreachable = unreachable_lanes.len();
        for l in unreachable_lanes {
            colorer.add_l(l, unreachable_color, map);
            println!("{} is unreachable", l);
        }

        Some(Box::new(Floodfiller {
            composite: WrappedComposite::quick_menu(
                ctx,
                title,
                vec![format!("{} unreachable lanes", num_unreachable)],
                vec![],
            ),
            colorer: colorer.build(ctx, ui),
        }))
    }
}

impl State for Floodfiller {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.colorer.draw(g);
        self.composite.draw(g);
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

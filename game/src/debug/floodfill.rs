use crate::app::App;
use crate::common::Colorer;
use crate::game::{State, Transition};
use crate::managed::WrappedComposite;
use ezgui::{Color, Composite, EventCtx, GfxCtx, Outcome};
use map_model::{connectivity, LaneID, Map, PathConstraints};
use std::collections::HashSet;

pub struct Floodfiller {
    composite: Composite,
    colorer: Colorer,
}

impl Floodfiller {
    pub fn floodfill(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State> {
        let (r, u, t) = find_reachable_from(l, &app.primary.map);
        Floodfiller::new(ctx, app, r, u, t)
    }
    pub fn scc(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State> {
        let constraints = PathConstraints::from_lt(app.primary.map.get_l(l).lane_type);
        let (good, bad) = connectivity::find_scc(&app.primary.map, constraints);
        Floodfiller::new(
            ctx,
            app,
            good,
            bad,
            format!("strongly-connected components for {:?}", constraints),
        )
    }

    fn new(
        ctx: &mut EventCtx,
        app: &App,
        reachable_lanes: HashSet<LaneID>,
        unreachable_lanes: HashSet<LaneID>,
        title: String,
    ) -> Box<dyn State> {
        let map = &app.primary.map;
        // Localized and debug
        let reachable_color = Color::GREEN;
        let unreachable_color = Color::RED;

        let mut colorer = Colorer::discrete(
            ctx,
            "Lane connectivity",
            Vec::new(),
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

        Box::new(Floodfiller {
            composite: WrappedComposite::quick_menu(
                ctx,
                app,
                title,
                vec![format!("{} unreachable lanes", num_unreachable)],
                vec![],
            ),
            colorer: colorer.build_both(ctx, app),
        })
    }
}

impl State for Floodfiller {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g, app);
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

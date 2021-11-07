use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};

use geom::Duration;
use map_gui::tools::PopupMsg;
use map_model::{
    connectivity, DirectedRoadID, IntersectionID, Map, PathConstraints, PathRequest, PathV2,
    RoadID, NORMAL_LANE_THICKNESS,
};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use super::Neighborhood;
use crate::app::{App, Transition};

struct RatRun {
    shortcut_path: PathV2,
    /// May be the same as the shortcut
    fastest_path: PathV2,
}

/// Ideally this returns every possible path through the neighborhood between two borders. Doesn't
/// work correctly yet.
fn find_rat_runs(
    map: &Map,
    neighborhood: &Neighborhood,
    modal_filters: &BTreeSet<RoadID>,
) -> Vec<RatRun> {
    let mut results: Vec<RatRun> = Vec::new();
    for i in &neighborhood.borders {
        let mut started_from: HashSet<DirectedRoadID> = HashSet::new();
        for l in map.get_i(*i).get_outgoing_lanes(map, PathConstraints::Car) {
            let dr = map.get_l(l).get_directed_parent();
            if !started_from.contains(&dr) && neighborhood.orig_perimeter.interior.contains(&dr.id)
            {
                started_from.insert(dr);
                results.extend(find_rat_runs_from(
                    map,
                    dr,
                    &neighborhood.borders,
                    modal_filters,
                ));
            }
        }
    }
    results.sort_by(|a, b| a.time_ratio().partial_cmp(&b.time_ratio()).unwrap());
    results
}

fn find_rat_runs_from(
    map: &Map,
    start: DirectedRoadID,
    borders: &BTreeSet<IntersectionID>,
    modal_filters: &BTreeSet<RoadID>,
) -> Vec<RatRun> {
    // If there's a filter where we're starting, we can't go anywhere
    if modal_filters.contains(&start.id) {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut back_refs = HashMap::new();
    let mut queue: BinaryHeap<Item> = BinaryHeap::new();
    queue.push(Item {
        node: start,
        cost: Duration::ZERO,
    });
    let mut visited = HashSet::new();

    while let Some(current) = queue.pop() {
        if visited.contains(&current.node) {
            continue;
        }
        visited.insert(current.node);

        // If we found a border, then stitch together the path
        let dst_i = current.node.dst_i(map);
        if borders.contains(&dst_i) {
            let mut at = current.node;
            let mut path = vec![at];
            while let Some(prev) = back_refs.get(&at).cloned() {
                path.push(prev);
                at = prev;
            }
            path.push(start);
            path.reverse();
            results.push(RatRun::new(map, path, current.cost));
            // TODO Keep searching for more, but infinite loop currently
            return results;
        }

        for mvmnt in map.get_movements_for(current.node, PathConstraints::Car) {
            // Can't cross filters
            if modal_filters.contains(&mvmnt.to.id) {
                continue;
            }

            queue.push(Item {
                cost: current.cost
                    + connectivity::vehicle_cost(
                        mvmnt.from,
                        mvmnt,
                        PathConstraints::Car,
                        map.routing_params(),
                        map,
                    )
                    + connectivity::zone_cost(mvmnt, PathConstraints::Car, map),
                node: mvmnt.to,
            });
            back_refs.insert(mvmnt.to, mvmnt.from);
        }
    }

    results
}

#[derive(PartialEq, Eq)]
struct Item {
    cost: Duration,
    node: DirectedRoadID,
}
impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Item) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Item) -> Ordering {
        // BinaryHeap is a max-heap, so reverse the comparison to get smallest times first.
        let ord = other.cost.cmp(&self.cost);
        if ord != Ordering::Equal {
            return ord;
        }
        self.node.cmp(&other.node)
    }
}

impl RatRun {
    fn new(map: &Map, path: Vec<DirectedRoadID>, cost: Duration) -> RatRun {
        // TODO This is flat out wrong. We need to find a "reasonable" from/to road, just outside
        // the neighborhood. Ideally on the perimeter, in a direction not forcing a U-turn.
        let req = PathRequest::between_directed_roads(
            map,
            path[0],
            *path.last().unwrap(),
            PathConstraints::Car,
        )
        .unwrap();
        let shortcut_path = PathV2::from_roads(
            path,
            req.clone(),
            cost,
            // TODO We're assuming there are no uber turns. Seems unlikely in the interior of a
            // neighborhood!
            Vec::new(),
            map,
        );
        let fastest_path = map.pathfind_v2(req).unwrap();
        // TODO If the path matches up, double check the cost does too, since we may calculate it
        // differently...
        RatRun {
            shortcut_path,
            fastest_path,
        }
    }

    /// The ratio of the shortcut's time to the fastest path's time. Smaller values mean the
    /// shortcut is more desirable.
    fn time_ratio(&self) -> f64 {
        // TODO Not sure why yet, just avoid crashing
        if self.fastest_path.get_cost() == Duration::ZERO {
            return 1.0;
        }

        self.shortcut_path.get_cost() / self.fastest_path.get_cost()
    }
}

pub struct BrowseRatRuns {
    panel: Panel,
    rat_runs: Vec<RatRun>,
    current_idx: usize,

    draw_paths: ToggleZoomed,
}

impl BrowseRatRuns {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        neighborhood: &Neighborhood,
    ) -> Box<dyn State<App>> {
        let rat_runs = find_rat_runs(&app.primary.map, neighborhood, &app.session.modal_filters);
        if rat_runs.is_empty() {
            return PopupMsg::new_state(ctx, "No rat runs detected", vec![""]);
        }

        let mut state = BrowseRatRuns {
            panel: Panel::empty(ctx),
            rat_runs,
            current_idx: 0,
            draw_paths: ToggleZoomed::empty(ctx),
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        let current = &self.rat_runs[self.current_idx];

        self.panel = Panel::new_builder(Widget::col(vec![
            ctx.style()
                .btn_outline
                .text("Back to editing modal filters")
                .hotkey(Key::Escape)
                .build_def(ctx),
            Line("Warning: placeholder results")
                .fg(Color::RED)
                .into_widget(ctx),
            Widget::row(vec![
                "Rat runs:".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_prev()
                    .disabled(self.current_idx == 0)
                    .hotkey(Key::LeftArrow)
                    .build_widget(ctx, "previous rat run"),
                Text::from(
                    Line(format!("{}/{}", self.current_idx + 1, self.rat_runs.len())).secondary(),
                )
                .into_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_next()
                    .disabled(self.current_idx == self.rat_runs.len() - 1)
                    .hotkey(Key::RightArrow)
                    .build_widget(ctx, "next rat run"),
            ]),
            Text::from_multiline(vec![
                Line(format!("Ratio: {:.2}", current.time_ratio())),
                Line(format!(
                    "Shortcut takes: {}",
                    current.shortcut_path.get_cost()
                )),
                Line(format!(
                    "Fastest path takes: {}",
                    current.fastest_path.get_cost()
                )),
            ])
            .into_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        // TODO Transforming into PathV1 seems like a particularly unnecessary step. Time to come
        // up with a native v2 drawing?
        let mut draw_paths = ToggleZoomed::builder();
        for (path, color) in [
            (current.shortcut_path.clone(), Color::RED),
            (current.fastest_path.clone(), Color::BLUE),
        ] {
            if let Ok(path) = path.into_v1(&app.primary.map) {
                if let Some(pl) = path.trace(&app.primary.map) {
                    let shape = pl.make_polygons(5.0 * NORMAL_LANE_THICKNESS);
                    draw_paths.unzoomed.push(color.alpha(0.8), shape.clone());
                    draw_paths.zoomed.push(color.alpha(0.5), shape);
                }
            }
        }
        self.draw_paths = draw_paths.build(ctx);
    }
}

impl State<App> for BrowseRatRuns {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Back to editing modal filters" => {
                    return Transition::Pop;
                }
                "previous rat run" => {
                    self.current_idx -= 1;
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    self.current_idx += 1;
                    self.recalculate(ctx, app);
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.draw_paths.draw(g);
        // TODO Draw everything from the previous state too... fade, the cells, filters, labels
    }
}

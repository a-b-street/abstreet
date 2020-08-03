use crate::app::App;
use crate::common::ColorDiscrete;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Choice, Color, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, TextExt, VerticalAlignment, Widget,
};
use map_model::{connectivity, LaneID, Map, PathConstraints};
use std::collections::HashSet;

pub struct Floodfiller {
    composite: Composite,
    unzoomed: Drawable,
    zoomed: Drawable,
    source: Source,
    constraints: PathConstraints,
}

impl Floodfiller {
    pub fn floodfill(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State> {
        let constraints = PathConstraints::from_lt(app.primary.map.get_l(l).lane_type);
        Floodfiller::new(ctx, app, Source::Floodfill(l), constraints)
    }
    pub fn scc(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State> {
        let constraints = PathConstraints::from_lt(app.primary.map.get_l(l).lane_type);
        Floodfiller::new(ctx, app, Source::SCC, constraints)
    }

    fn new(
        ctx: &mut EventCtx,
        app: &App,
        source: Source,
        constraints: PathConstraints,
    ) -> Box<dyn State> {
        let (reachable_lanes, unreachable_lanes, title) =
            source.calculate(&app.primary.map, constraints);
        let mut colorer = ColorDiscrete::new(
            app,
            vec![("unreachable", Color::RED), ("reachable", Color::GREEN)],
        );
        for l in reachable_lanes {
            colorer.add_l(l, "reachable");
        }
        let num_unreachable = unreachable_lanes.len();
        for l in unreachable_lanes {
            colorer.add_l(l, "unreachable");
        }

        let (unzoomed, zoomed, legend) = colorer.build(ctx);
        Box::new(Floodfiller {
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line(title).small_heading().draw(ctx),
                    Btn::text_fg("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                format!("{} unreachable lanes", num_unreachable).draw_text(ctx),
                legend,
                Widget::row(vec![
                    "Connectivity type:".draw_text(ctx),
                    Widget::dropdown(
                        ctx,
                        "constraints",
                        constraints,
                        PathConstraints::all()
                            .into_iter()
                            .map(|c| Choice::new(format!("{:?}", c), c))
                            .collect(),
                    ),
                ]),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            unzoomed,
            zoomed,
            source,
            constraints,
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
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        let constraints = self.composite.dropdown_value("constraints");
        if constraints != self.constraints {
            return Transition::Replace(Floodfiller::new(
                ctx,
                app,
                self.source.clone(),
                constraints,
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        self.composite.draw(g);
    }
}

#[derive(Clone)]
enum Source {
    Floodfill(LaneID),
    SCC,
}

impl Source {
    // (reachable, unreachable, a title)
    fn calculate(
        &self,
        map: &Map,
        constraints: PathConstraints,
    ) -> (HashSet<LaneID>, HashSet<LaneID>, String) {
        match self {
            Source::Floodfill(start) => {
                let mut visited = HashSet::new();
                let mut queue = vec![*start];
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

                (visited, unreached, format!("Floodfill from {}", start))
            }
            Source::SCC => {
                let (good, bad) = connectivity::find_scc(map, constraints);
                (good, bad, format!("strongpy-connected component"))
            }
        }
    }
}

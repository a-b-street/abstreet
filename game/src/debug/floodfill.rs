use std::collections::HashSet;

use map_gui::tools::ColorDiscrete;
use map_model::{connectivity, LaneID, Map, PathConstraints};
use widgetry::{
    Choice, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State,
    StyledButtons, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;

pub struct Floodfiller {
    panel: Panel,
    unzoomed: Drawable,
    zoomed: Drawable,
    source: Source,
}

impl Floodfiller {
    pub fn floodfill(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State<App>> {
        let constraints = PathConstraints::from_lt(app.primary.map.get_l(l).lane_type);
        Floodfiller::new(ctx, app, Source::Floodfill(l), constraints)
    }
    pub fn scc(ctx: &mut EventCtx, app: &App, l: LaneID) -> Box<dyn State<App>> {
        let constraints = PathConstraints::from_lt(app.primary.map.get_l(l).lane_type);
        Floodfiller::new(ctx, app, Source::SCC, constraints)
    }

    fn new(
        ctx: &mut EventCtx,
        app: &App,
        source: Source,
        constraints: PathConstraints,
    ) -> Box<dyn State<App>> {
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
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line(title).small_heading().draw(ctx),
                    ctx.style().btn_close_widget(ctx),
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
        })
    }
}

impl State<App> for Floodfiller {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                return Transition::Replace(Floodfiller::new(
                    ctx,
                    app,
                    self.source.clone(),
                    self.panel.dropdown_value("constraints"),
                ));
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        self.panel.draw(g);
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

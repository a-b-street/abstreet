use crate::app::App;
use crate::common::ColorDiscrete;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, TextExt, VerticalAlignment, Widget,
};
use map_model::{connectivity, LaneID, Map, PathConstraints};
use std::collections::HashSet;

pub struct Floodfiller {
    composite: Composite,
    unzoomed: Drawable,
    zoomed: Drawable,
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
            println!("{} is unreachable", l);
        }

        let (unzoomed, zoomed, legend) = colorer.build(ctx);
        Box::new(Floodfiller {
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line(title).small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    format!("{} unreachable lanes", num_unreachable).draw_text(ctx),
                    legend,
                ])
                .padding(16)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            unzoomed,
            zoomed,
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
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
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

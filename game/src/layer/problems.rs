use std::collections::BTreeSet;

use geom::{Circle, Distance, Pt2D, Time};
use map_gui::tools::{make_heatmap, HeatmapOptions};
use sim::{Problem, TripInfo, TripMode};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Outcome, Panel, Toggle, Widget};

use crate::app::App;
use crate::common::checkbox_per_mode;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct ProblemMap {
    time: Time,
    opts: Options,
    draw: Drawable,
    panel: Panel,
}

impl Layer for ProblemMap {
    fn name(&self) -> Option<&'static str> {
        Some("problem map")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            let mut new = ProblemMap::new(ctx, app, self.opts.clone());
            new.panel.restore(ctx, &self.panel);
            *self = new;
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            _ => {
                let new_opts = self.options();
                if self.opts != new_opts {
                    *self = ProblemMap::new(ctx, app, new_opts);
                }
            }
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.draw);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw);
    }
}

impl ProblemMap {
    pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> ProblemMap {
        let mut pts = Vec::new();
        for (trip, problems) in &app.primary.sim.get_analytics().problems_per_trip {
            for problem in problems {
                if opts.show(app.primary.sim.trip_info(*trip), problem) {
                    pts.push(match problem {
                        Problem::IntersectionDelay(i, _)
                        | Problem::LargeIntersectionCrossing(i) => {
                            app.primary.map.get_i(*i).polygon.center()
                        }
                        Problem::OvertakeDesired(on) => on.get_polyline(&app.primary.map).middle(),
                    });
                }
            }
        }

        let mut batch = GeomBatch::new();
        let legend = if let Some(ref o) = opts.heatmap {
            Some(make_heatmap(
                ctx,
                &mut batch,
                app.primary.map.get_bounds(),
                pts,
                o,
            ))
        } else {
            let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(10.0)).to_polygon();
            // TODO Different colors per problem type
            for pt in pts {
                batch.push(Color::RED.alpha(0.8), circle.translate(pt.x(), pt.y()));
            }
            None
        };
        let controls = make_controls(ctx, app, &opts, legend);
        ProblemMap {
            time: app.primary.sim.time(),
            opts,
            draw: ctx.upload(batch),
            panel: controls,
        }
    }

    fn options(&self) -> Options {
        let heatmap = if self.panel.is_checked("Show heatmap") {
            Some(HeatmapOptions::from_controls(&self.panel))
        } else {
            None
        };
        let mut modes = BTreeSet::new();
        for m in TripMode::all() {
            if self.panel.is_checked(m.ongoing_verb()) {
                modes.insert(m);
            }
        }
        Options {
            heatmap,
            modes,
            show_delays: self.panel.is_checked("show delays"),
            show_large_crossings: self
                .panel
                .is_checked("show crossings over large intersections"),
            show_overtakes: self
                .panel
                .is_checked("show where cars want to overtake cyclists"),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Options {
    // If None, just a dot map
    heatmap: Option<HeatmapOptions>,
    modes: BTreeSet<TripMode>,
    show_delays: bool,
    show_large_crossings: bool,
    show_overtakes: bool,
    // TODO Time range
}

impl Options {
    pub fn new() -> Options {
        Options {
            heatmap: None,
            modes: TripMode::all().into_iter().collect(),
            show_delays: true,
            show_large_crossings: true,
            show_overtakes: true,
        }
    }

    fn show(&self, trip: TripInfo, problem: &Problem) -> bool {
        if !self.modes.contains(&trip.mode) {
            return false;
        }
        match problem {
            Problem::IntersectionDelay(_, _) => self.show_delays,
            Problem::LargeIntersectionCrossing(_) => self.show_large_crossings,
            Problem::OvertakeDesired(_) => self.show_overtakes,
        }
    }
}

fn make_controls(ctx: &mut EventCtx, app: &App, opts: &Options, legend: Option<Widget>) -> Panel {
    let mut col = vec![header(ctx, "Problems encountered")];

    col.push(checkbox_per_mode(ctx, app, &opts.modes));
    col.push(Toggle::checkbox(ctx, "show delays", None, opts.show_delays));
    col.push(Toggle::checkbox(
        ctx,
        "show crossings over large intersections",
        None,
        opts.show_large_crossings,
    ));
    col.push(Toggle::checkbox(
        ctx,
        "show where cars want to overtake cyclists",
        None,
        opts.show_overtakes,
    ));

    col.push(Toggle::switch(
        ctx,
        "Show heatmap",
        None,
        opts.heatmap.is_some(),
    ));
    if let Some(ref o) = opts.heatmap {
        col.extend(o.to_controls(ctx, legend.unwrap()));
    }

    Panel::new(Widget::col(col))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx)
}

use std::collections::BTreeSet;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Pt2D, Time};
use map_gui::tools::{checkbox_per_mode, make_heatmap, HeatmapOptions};
use sim::{Problem, TripInfo};
use synthpop::TripMode;
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    Color, EventCtx, GfxCtx, Line, Outcome, Panel, PanelDims, Slider, Text, TextExt, Toggle, Widget,
};

use super::problems_diff::ProblemTypes;
use crate::app::App;
use crate::layer::{header, problems_diff, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct ProblemMap {
    time: Time,
    opts: Options,
    draw: ToggleZoomed,
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
            Outcome::Changed(x) => {
                if x == "Compare before proposal" {
                    return Some(LayerOutcome::Replace(Box::new(
                        problems_diff::RelativeProblemMap::new(ctx, app, self.opts.types.clone()),
                    )));
                }

                let new_opts = self.options(app);
                if self.opts != new_opts {
                    *self = ProblemMap::new(ctx, app, new_opts);
                }
            }
            _ => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.draw.draw(g);
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw.unzoomed);
    }
}

impl ProblemMap {
    pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> ProblemMap {
        let mut pts = Vec::new();
        for (trip, problems) in &app.primary.sim.get_analytics().problems_per_trip {
            for (time, problem) in problems {
                if opts.show(app.primary.sim.trip_info(*trip), *time, problem) {
                    pts.push(problem.point(&app.primary.map));
                }
            }
        }
        let num_pts = pts.len();

        let mut draw = ToggleZoomed::builder();
        let legend = if let Some(ref o) = opts.heatmap {
            Some(make_heatmap(
                ctx,
                &mut draw.unzoomed,
                app.primary.map.get_bounds(),
                pts,
                o,
            ))
        } else {
            let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(10.0)).to_polygon();
            // TODO Different colors per problem type
            for pt in pts {
                draw.unzoomed
                    .push(Color::PURPLE.alpha(0.8), circle.translate(pt.x(), pt.y()));
            }
            None
        };
        let controls = make_controls(ctx, app, &opts, legend, num_pts);
        ProblemMap {
            time: app.primary.sim.time(),
            opts,
            draw: draw.build(ctx),
            panel: controls,
        }
    }

    fn options(&self, app: &App) -> Options {
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
        let end_of_day = app.primary.sim.get_end_of_day();
        Options {
            heatmap,
            modes,
            time1: end_of_day.percent_of(self.panel.slider("time1").get_percent()),
            time2: end_of_day.percent_of(self.panel.slider("time2").get_percent()),
            types: ProblemTypes::from_controls(&self.panel),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Options {
    // If None, just a dot map
    heatmap: Option<HeatmapOptions>,
    modes: BTreeSet<TripMode>,
    time1: Time,
    time2: Time,
    pub types: ProblemTypes,
}

impl Options {
    pub fn new(app: &App) -> Self {
        Self {
            heatmap: Some(HeatmapOptions::new()),
            modes: TripMode::all().into_iter().collect(),
            time1: Time::START_OF_DAY,
            time2: app.primary.sim.get_end_of_day(),
            types: ProblemTypes::new(),
        }
    }

    fn show(&self, trip: TripInfo, time: Time, problem: &Problem) -> bool {
        if !self.modes.contains(&trip.mode) || time < self.time1 || time > self.time2 {
            return false;
        }
        self.types.show(problem)
    }
}

fn make_controls(
    ctx: &mut EventCtx,
    app: &App,
    opts: &Options,
    legend: Option<Widget>,
    num_problems: usize,
) -> Panel {
    let mut col = vec![
        header(ctx, "Problems encountered"),
        Text::from_all(vec![
            Line("Matching problems: ").secondary(),
            Line(prettyprint_usize(num_problems)),
        ])
        .into_widget(ctx),
    ];
    if app.has_prebaked().is_some() {
        col.push(Toggle::switch(ctx, "Compare before proposal", None, false));
    }

    // TODO You can't drag the sliders, since we don't remember that we're dragging a particular
    // slider when we recreate it here. Use panel.replace?
    let end_of_day = app.primary.sim.get_end_of_day();
    col.push(Widget::row(vec![
        "Happening between:".text_widget(ctx).margin_right(20),
        Slider::area(
            ctx,
            0.15 * ctx.canvas.window_width,
            opts.time1.to_percent(end_of_day),
            "time1",
        )
        .align_right(),
    ]));
    col.push(Widget::row(vec![
        "and:".text_widget(ctx).margin_right(20),
        Slider::area(
            ctx,
            0.15 * ctx.canvas.window_width,
            opts.time2.to_percent(end_of_day),
            "time2",
        )
        .align_right(),
    ]));
    col.push(checkbox_per_mode(ctx, app, &opts.modes));
    col.push(opts.types.to_controls(ctx));

    col.push(Toggle::choice(
        ctx,
        "Show heatmap",
        "Heatmap",
        "Points",
        None,
        opts.heatmap.is_some(),
    ));
    if let Some(ref o) = opts.heatmap {
        col.push(Line("Heatmap Options").small_heading().into_widget(ctx));
        col.extend(o.to_controls(ctx, legend.unwrap()));
    }

    Panel::new_builder(Widget::col(col))
        .aligned_pair(PANEL_PLACEMENT)
        // TODO Tune and use more widely
        .dims_height(PanelDims::MaxPercent(0.6))
        // TODO Not sure why needed -- if you leave the mouse on the right spot,
        // Outcome::Changed(time1) happens?
        .ignore_initial_events()
        .build(ctx)
}

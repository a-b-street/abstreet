use std::collections::BTreeSet;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Pt2D, Time};
use map_gui::tools::{make_heatmap, HeatmapOptions};
use sim::{Problem, TripInfo, TripMode};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, Slider, Text, TextExt,
    Toggle, Widget,
};

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
                let new_opts = self.options(app);
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
            for (time, problem) in problems {
                if opts.show(app.primary.sim.trip_info(*trip), *time, problem) {
                    pts.push(match problem {
                        Problem::IntersectionDelay(i, _)
                        | Problem::ComplexIntersectionCrossing(i) => {
                            app.primary.map.get_i(*i).polygon.center()
                        }
                        Problem::OvertakeDesired(on) => on.get_polyline(&app.primary.map).middle(),
                        Problem::ArterialIntersectionCrossing(t) => {
                            app.primary.map.get_t(*t).geom.middle()
                        }
                    });
                }
            }
        }
        let num_pts = pts.len();

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
                batch.push(Color::PURPLE.alpha(0.8), circle.translate(pt.x(), pt.y()));
            }
            None
        };
        let controls = make_controls(ctx, app, &opts, legend, num_pts);
        ProblemMap {
            time: app.primary.sim.time(),
            opts,
            draw: ctx.upload(batch),
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
            show_delays: self.panel.is_checked("show delays"),
            show_complex_crossings: self
                .panel
                .is_checked("show where cyclists cross complex intersections"),
            show_overtakes: self
                .panel
                .is_checked("show where cars want to overtake cyclists"),
            show_arterial_crossings: self
                .panel
                .is_checked("show where pedestrians cross arterial intersections"),
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
    show_delays: bool,
    show_complex_crossings: bool,
    show_overtakes: bool,
    show_arterial_crossings: bool,
    // TODO Time range
}

impl Options {
    pub fn new(app: &App) -> Options {
        Options {
            heatmap: Some(HeatmapOptions::new()),
            modes: TripMode::all().into_iter().collect(),
            time1: Time::START_OF_DAY,
            time2: app.primary.sim.get_end_of_day(),
            show_delays: true,
            show_complex_crossings: true,
            show_overtakes: true,
            show_arterial_crossings: true,
        }
    }

    fn show(&self, trip: TripInfo, time: Time, problem: &Problem) -> bool {
        if !self.modes.contains(&trip.mode) || time < self.time1 || time > self.time2 {
            return false;
        }
        match problem {
            Problem::IntersectionDelay(_, _) => self.show_delays,
            Problem::ComplexIntersectionCrossing(_) => self.show_complex_crossings,
            Problem::OvertakeDesired(_) => self.show_overtakes,
            Problem::ArterialIntersectionCrossing(_) => self.show_arterial_crossings,
        }
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

    // TODO You can't drag the sliders, since we don't remember that we're dragging a particular
    // slider when we recreate it here. Use panel.replace?
    let end_of_day = app.primary.sim.get_end_of_day();
    col.push(Widget::row(vec![
        "Happening between:".text_widget(ctx).margin_right(20),
        Slider::area(
            ctx,
            0.15 * ctx.canvas.window_width,
            opts.time1.to_percent(end_of_day),
        )
        .align_right()
        .named("time1"),
    ]));
    col.push(Widget::row(vec![
        "and:".text_widget(ctx).margin_right(20),
        Slider::area(
            ctx,
            0.15 * ctx.canvas.window_width,
            opts.time2.to_percent(end_of_day),
        )
        .align_right()
        .named("time2"),
    ]));
    col.push(checkbox_per_mode(ctx, app, &opts.modes));
    col.push(Toggle::checkbox(ctx, "show delays", None, opts.show_delays));
    col.push(Toggle::checkbox(
        ctx,
        "show where cyclists cross complex intersections",
        None,
        opts.show_complex_crossings,
    ));
    col.push(Toggle::checkbox(
        ctx,
        "show where cars want to overtake cyclists",
        None,
        opts.show_overtakes,
    ));
    col.push(Toggle::checkbox(
        ctx,
        "show where pedestrians cross wide intersections",
        None,
        opts.show_arterial_crossings,
    ));

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
        .build(ctx)
}

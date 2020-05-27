use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::color_for_mode;
use crate::sandbox::dashboards::DashTab;
use abstutil::prettyprint_usize;
use ezgui::{
    Checkbox, Choice, Color, Composite, DrawWithTooltips, EventCtx, GeomBatch, GfxCtx, Line,
    Outcome, ScatterPlot, Text, TextExt, Widget,
};
use geom::{Distance, Duration, Polygon, Pt2D};
use sim::TripMode;
use std::collections::BTreeSet;

pub struct TripSummaries {
    composite: Composite,
    filter: Filter,
}

impl TripSummaries {
    pub fn new(ctx: &mut EventCtx, app: &App, filter: Filter) -> Box<dyn State> {
        let mut filters = vec![Widget::dropdown(
            ctx,
            "filter",
            filter.changes_pct,
            vec![
                Choice::new("any change", None),
                Choice::new("at least 1% change", Some(0.01)),
                Choice::new("at least 10% change", Some(0.1)),
                Choice::new("at least 50% change", Some(0.5)),
            ],
        )
        .margin_right(10)];
        for m in TripMode::all() {
            filters.push(
                Checkbox::colored(
                    ctx,
                    m.ongoing_verb(),
                    color_for_mode(app, m),
                    filter.modes.contains(&m),
                )
                .margin_right(5),
            );
            filters.push(m.ongoing_verb().draw_text(ctx).margin_right(10));
        }

        Box::new(TripSummaries {
            composite: Composite::new(
                Widget::col(vec![
                    DashTab::TripSummaries.picker(ctx, app),
                    Widget::row(filters).centered_horiz().margin_below(10),
                    summary(ctx, app, &filter).margin_below(10),
                    Widget::row(vec![
                        contingency_table(ctx, app, &filter)
                            .centered_vert()
                            .margin_right(20),
                        scatter_plot(ctx, app, &filter),
                    ])
                    .evenly_spaced(),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .exact_size_percent(90, 90)
            .build(ctx),
            filter,
        })
    }
}

impl State for TripSummaries {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => DashTab::TripSummaries.transition(ctx, app, &x),
            None => {
                let mut filter = Filter {
                    changes_pct: self.composite.dropdown_value("filter"),
                    modes: BTreeSet::new(),
                };
                for m in TripMode::all() {
                    if self.composite.is_checked(m.ongoing_verb()) {
                        filter.modes.insert(m);
                    }
                }
                if filter != self.filter {
                    Transition::Replace(TripSummaries::new(ctx, app, filter))
                } else {
                    Transition::Keep
                }
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}

fn summary(ctx: &mut EventCtx, app: &App, filter: &Filter) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let mut num_same = 0;
    let mut num_faster = 0;
    let mut num_slower = 0;
    let mut sum_faster = Duration::ZERO;
    let mut sum_slower = Duration::ZERO;
    for (b, a, mode) in app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked())
    {
        if !filter.modes.contains(&mode) {
            continue;
        }
        let same = if let Some(pct) = filter.changes_pct {
            pct_diff(a, b) <= pct
        } else {
            a == b
        };

        if same {
            num_same += 1;
        } else if a < b {
            num_faster += 1;
            sum_faster += b - a;
        } else {
            num_slower += 1;
            sum_slower += a - b;
        }
    }

    Widget::col(vec![Widget::row(vec![
        Widget::col(vec![Text::from_multiline(vec![
            Line(format!("{} trips faster", prettyprint_usize(num_faster))),
            Line(format!("{} total time saved", sum_faster)),
            Line(format!(
                "Average {} per faster trip",
                if num_faster == 0 {
                    Duration::ZERO
                } else {
                    sum_faster / (num_faster as f64)
                }
            )),
        ])
        .draw(ctx)])
        .outline(2.0, Color::GREEN)
        .padding(10),
        Line(format!("{} trips unchanged", prettyprint_usize(num_same)))
            .draw(ctx)
            .centered_vert()
            .margin_horiz(5)
            .outline(2.0, Color::YELLOW)
            .padding(10),
        Widget::col(vec![Text::from_multiline(vec![
            Line(format!("{} trips slower", prettyprint_usize(num_slower))),
            Line(format!("{} total time lost", sum_slower)),
            Line(format!(
                "Average {} per slower trip",
                if num_slower == 0 {
                    Duration::ZERO
                } else {
                    sum_slower / (num_slower as f64)
                }
            )),
        ])
        .draw(ctx)])
        .outline(2.0, Color::RED)
        .padding(10),
    ])
    .evenly_spaced()])
}

fn scatter_plot(ctx: &mut EventCtx, app: &App, filter: &Filter) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let points = filter.get_trips(app);
    if points.is_empty() {
        return Widget::nothing();
    }

    ScatterPlot::new(
        ctx,
        "Trip time before changes",
        "Trip time after changes",
        points,
    )
    .outline(2.0, Color::WHITE)
    .padding(10)
}

fn contingency_table(ctx: &mut EventCtx, app: &App, filter: &Filter) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let total_width = 500.0;
    let total_height = 300.0;

    let points = filter.get_trips(app);
    if points.is_empty() {
        return Widget::nothing();
    }
    let num_buckets = 10;
    let (_, endpts) = points
        .iter()
        .map(|(b, a)| a.max(b))
        .max()
        .unwrap()
        .make_intervals_for_max(num_buckets);

    let mut batch = GeomBatch::new();
    batch.autocrop_dims = false;

    // Draw the X axis, time before changes in buckets.
    for (idx, mins) in endpts.iter().enumerate() {
        batch.add_centered(
            Text::from(Line(mins.to_string()).small()).render_ctx(ctx),
            Pt2D::new(
                (idx as f64) / (num_buckets as f64) * total_width,
                total_height / 2.0,
            ),
        );
    }

    // Now measure savings and losses per bucket.
    let mut savings_per_bucket: Vec<(Duration, usize)> = std::iter::repeat((Duration::ZERO, 0))
        .take(num_buckets)
        .collect();
    let mut losses_per_bucket: Vec<(Duration, usize)> = std::iter::repeat((Duration::ZERO, 0))
        .take(num_buckets)
        .collect();
    for (b, a) in points {
        let before_mins = b.num_minutes_rounded_up();
        let raw_idx = endpts.iter().rev().position(|x| before_mins >= *x).unwrap();
        let mut idx = endpts.len() - 1 - raw_idx;
        // Careful. We might be exactly the max...
        if idx == endpts.len() - 1 {
            idx -= 1;
        }

        if a > b {
            losses_per_bucket[idx].0 += a - b;
            losses_per_bucket[idx].1 += 1;
        } else if a < b {
            savings_per_bucket[idx].0 += b - a;
            savings_per_bucket[idx].1 += 1;
        }
    }
    let max_y = losses_per_bucket
        .iter()
        .max()
        .unwrap()
        .0
        .max(savings_per_bucket.iter().max().unwrap().0);

    // Draw the bars!
    let bar_width = total_width / (num_buckets as f64);
    let max_bar_height = (total_height - ctx.default_line_height()) / 2.0;
    let mut outlines = Vec::new();
    let mut tooltips = Vec::new();
    let mut x1 = 0.0;
    let mut idx = 0;
    for ((total_savings, num_savings), (total_loss, num_loss)) in savings_per_bucket
        .into_iter()
        .zip(losses_per_bucket.into_iter())
    {
        if num_savings > 0 {
            let height = (total_savings / max_y) * max_bar_height;
            let rect = Polygon::rectangle(bar_width, height).translate(x1, max_bar_height - height);
            if let Some(o) = rect.maybe_to_outline(Distance::meters(1.5)) {
                outlines.push(o);
            }
            batch.push(Color::GREEN, rect.clone());
            tooltips.push((
                rect,
                Text::from_multiline(vec![
                    Line(format!(
                        "{} trips between {} and {} minutes",
                        prettyprint_usize(num_savings),
                        endpts[idx],
                        endpts[idx + 1]
                    )),
                    Line(format!("Total savings: {}", total_savings)),
                ]),
            ));
        }
        if num_loss > 0 {
            let height = (total_loss / max_y) * max_bar_height;
            let rect =
                Polygon::rectangle(bar_width, height).translate(x1, total_height - max_bar_height);
            if let Some(o) = rect.maybe_to_outline(Distance::meters(1.5)) {
                outlines.push(o);
            }
            batch.push(Color::RED, rect.clone());
            tooltips.push((
                rect,
                Text::from_multiline(vec![
                    Line(format!(
                        "{} trips between {} and {} minutes",
                        prettyprint_usize(num_loss),
                        endpts[idx],
                        endpts[idx + 1]
                    )),
                    Line(format!("Total losses: {}", total_loss)),
                ]),
            ));
        }
        x1 += bar_width;
        idx += 1;
    }
    batch.extend(Color::BLACK, outlines);

    Widget::row(vec![DrawWithTooltips::new(ctx, batch, tooltips)])
        .outline(2.0, Color::WHITE)
        .padding(10)
}

#[derive(PartialEq)]
pub struct Filter {
    changes_pct: Option<f64>,
    modes: BTreeSet<TripMode>,
}

impl Filter {
    pub fn new() -> Filter {
        Filter {
            changes_pct: None,
            modes: TripMode::all().into_iter().collect(),
        }
    }

    fn get_trips(&self, app: &App) -> Vec<(Duration, Duration)> {
        let mut points = Vec::new();
        for (b, a, mode) in app
            .primary
            .sim
            .get_analytics()
            .both_finished_trips(app.primary.sim.time(), app.prebaked())
        {
            if self.modes.contains(&mode)
                && self
                    .changes_pct
                    .map(|pct| pct_diff(a, b) > pct)
                    .unwrap_or(true)
            {
                points.push((b, a));
            }
        }
        points
    }
}

fn pct_diff(a: Duration, b: Duration) -> f64 {
    if a >= b {
        (a / b) - 1.0
    } else {
        (b / a) - 1.0
    }
}

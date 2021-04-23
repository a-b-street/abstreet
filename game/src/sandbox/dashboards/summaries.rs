use std::collections::BTreeSet;
use std::fs::File;
use std::io::Write;

use anyhow::Result;

use abstutil::prettyprint_usize;
use geom::{Distance, Duration, Polygon, Pt2D};
use map_gui::tools::PopupMsg;
use sim::TripMode;
use widgetry::{
    Choice, Color, CompareTimes, DrawWithTooltips, EventCtx, GeomBatch, GfxCtx, Line, Outcome,
    Panel, State, Text, TextExt, Toggle, Widget,
};

use crate::app::{App, Transition};
use crate::common::color_for_mode;
use crate::sandbox::dashboards::DashTab;

pub struct TripSummaries {
    panel: Panel,
}

impl TripSummaries {
    pub fn new(ctx: &mut EventCtx, app: &App, filter: Filter) -> Box<dyn State<App>> {
        let mut filters = vec!["Filters".text_widget(ctx)];
        for mode in TripMode::all() {
            filters.push(Toggle::colored_checkbox(
                ctx,
                mode.ongoing_verb(),
                color_for_mode(app, mode),
                filter.modes.contains(&mode),
            ));
        }
        filters.push(Widget::dropdown(
            ctx,
            "filter",
            filter.changes_pct,
            vec![
                Choice::new("any change", None),
                Choice::new("at least 1% change", Some(0.01)),
                Choice::new("at least 10% change", Some(0.1)),
                Choice::new("at least 50% change", Some(0.5)),
            ],
        ));
        filters.push(
            ctx.style()
                .btn_plain
                .text("Export to CSV")
                .build_def(ctx)
                .align_bottom(),
        );

        Box::new(TripSummaries {
            panel: Panel::new(Widget::col(vec![
                DashTab::TripSummaries.picker(ctx, app),
                Widget::row(vec![
                    Widget::col(filters).section(ctx),
                    Widget::col(vec![
                        summary_boxes(ctx, app, &filter),
                        Widget::row(vec![
                            contingency_table(ctx, app, &filter).bg(ctx.style().section_bg),
                            scatter_plot(ctx, app, &filter).bg(ctx.style().section_bg),
                        ])
                        .evenly_spaced(),
                    ]),
                ]),
            ]))
            .exact_size_percent(90, 90)
            .build(ctx),
        })
    }
}

impl State<App> for TripSummaries {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Export to CSV" => {
                    return Transition::Push(match export_times(app) {
                        Ok(path) => PopupMsg::new(
                            ctx,
                            "Data exported",
                            vec![format!("Data exported to {}", path)],
                        ),
                        Err(err) => PopupMsg::new(ctx, "Export failed", vec![err.to_string()]),
                    });
                }
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                if let Some(t) = DashTab::TripSummaries.transition(ctx, app, &self.panel) {
                    return t;
                }

                let mut filter = Filter {
                    changes_pct: self.panel.dropdown_value("filter"),
                    modes: BTreeSet::new(),
                };
                for m in TripMode::all() {
                    if self.panel.is_checked(m.ongoing_verb()) {
                        filter.modes.insert(m);
                    }
                }
                Transition::Replace(TripSummaries::new(ctx, app, filter))
            }
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _app: &App) {
        self.panel.draw(g);
    }
}

fn summary_boxes(ctx: &mut EventCtx, app: &App, filter: &Filter) -> Widget {
    let mut num_same = 0;
    let mut num_faster = 0;
    let mut num_slower = 0;
    let mut sum_faster = Duration::ZERO;
    let mut sum_slower = Duration::ZERO;
    for (_, b, a, mode) in app
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
    let num_total = (num_faster + num_slower + num_same) as f64;

    Widget::row(vec![
        Text::from_multiline(vec![
            Line(format!("Faster Trips: {}", prettyprint_usize(num_faster))).big_heading_plain(),
            Line(format!(
                "{:.2}% of finished trips",
                100.0 * (num_faster as f64) / num_total
            ))
            .small(),
            Line(format!(
                "Average {} faster per trip",
                if num_faster == 0 {
                    Duration::ZERO
                } else {
                    sum_faster / (num_faster as f64)
                }
            ))
            .small(),
            Line(format!("Saved {} in total", sum_faster)).small(),
        ])
        .into_widget(ctx)
        .container()
        .padding(20)
        .bg(Color::hex("#72CE36").alpha(0.5))
        .outline(ctx.style().section_outline),
        Text::from_multiline(vec![
            Line(format!("Slower Trips: {}", prettyprint_usize(num_slower))).big_heading_plain(),
            Line(format!(
                "{:.2}% of finished trips",
                100.0 * (num_slower as f64) / num_total
            ))
            .small(),
            Line(format!(
                "Average {} slower per trip",
                if num_slower == 0 {
                    Duration::ZERO
                } else {
                    sum_slower / (num_slower as f64)
                }
            ))
            .small(),
            Line(format!("Lost {} in total", sum_slower)).small(),
        ])
        .into_widget(ctx)
        .container()
        .padding(20)
        .bg(Color::hex("#EB3223").alpha(0.5))
        .outline(ctx.style().section_outline),
        Text::from_multiline(vec![
            Line(format!("Unchanged: {}", prettyprint_usize(num_same))).big_heading_plain(),
            Line(format!(
                "{:.2}% of finished trips",
                100.0 * (num_same as f64) / num_total
            ))
            .small(),
        ])
        .into_widget(ctx)
        .container()
        .padding(20)
        .bg(Color::hex("#F4DA22").alpha(0.5))
        .outline(ctx.style().section_outline),
    ])
    .evenly_spaced()
}

fn scatter_plot(ctx: &mut EventCtx, app: &App, filter: &Filter) -> Widget {
    let points = filter.get_trips(app);
    if points.is_empty() {
        return Widget::nothing();
    }

    Widget::col(vec![
        Line("Trip time before and after")
            .small_heading()
            .into_widget(ctx),
        CompareTimes::new(
            ctx,
            format!(
                "Trip time before \"{}\"",
                app.primary.map.get_edits().edits_name
            ),
            format!(
                "Trip time after \"{}\"",
                app.primary.map.get_edits().edits_name
            ),
            points,
        ),
    ])
    .padding(16)
    .outline(ctx.style().section_outline)
}

fn contingency_table(ctx: &mut EventCtx, app: &App, filter: &Filter) -> Widget {
    let total_width = 500.0;
    let total_height = 300.0;

    let points = filter.get_trips(app);
    if points.is_empty() {
        return Widget::nothing();
    }

    // bucket by trip duration _before_ changes
    let num_buckets = 10;
    let (_, endpts) = points
        .iter()
        .map(|(b, _a)| b)
        .max()
        .unwrap()
        .make_intervals_for_max(num_buckets);

    let mut batch = GeomBatch::new();
    batch.autocrop_dims = false;

    // Draw the X axis
    for (idx, mins) in endpts.iter().enumerate() {
        batch.append(
            Text::from(Line(mins.to_string()).secondary())
                .render(ctx)
                .centered_on(Pt2D::new(
                    (idx as f64) / (num_buckets as f64) * total_width,
                    total_height / 2.0,
                )),
        );
    }
    // TODO Position this better
    if false {
        batch.append(
            Text::from_multiline(vec![
                Line("trip").secondary(),
                Line("time").secondary(),
                Line("after").secondary(),
            ])
            .render(ctx)
            .translate(total_width, total_height / 2.0),
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
        // bucket by trip duration _before_ changes
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
            if let Ok(o) = rect.to_outline(Distance::meters(1.5)) {
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
                    Line(format!("Saved {} in total", total_savings)).fg(Color::hex("#72CE36")),
                ]),
            ));
        }
        if num_loss > 0 {
            let height = (total_loss / max_y) * max_bar_height;
            let rect =
                Polygon::rectangle(bar_width, height).translate(x1, total_height - max_bar_height);
            if let Ok(o) = rect.to_outline(Distance::meters(1.5)) {
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
                    Line(format!("Lost {} in total", total_loss)).fg(Color::hex("#EB3223")),
                ]),
            ));
        }
        x1 += bar_width;
        idx += 1;
    }
    batch.extend(Color::BLACK, outlines);

    Widget::col(vec![
        Text::from_multiline(vec![
            Line("Time difference by trip length").small_heading(),
            Line("Grouped by the length of the trip before your changes."),
        ])
        .into_widget(ctx),
        Line("Total Time Saved (faster)")
            .secondary()
            .into_widget(ctx),
        DrawWithTooltips::new(ctx, batch, tooltips, Box::new(|_| GeomBatch::new())),
        Line("Total Time Lost (slower)")
            .secondary()
            .into_widget(ctx),
    ])
    .padding(16)
    .outline(ctx.style().section_outline)
}

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
        for (_, b, a, mode) in app
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

fn export_times(app: &App) -> Result<String> {
    let path = format!(
        "trip_times_{}_{}.csv",
        app.primary.map.get_name().as_filename(),
        app.primary.sim.time().as_filename()
    );
    let mut f = File::create(&path)?;
    writeln!(f, "id,mode,seconds_before,seconds_after")?;
    for (id, b, a, mode) in app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked())
    {
        writeln!(
            f,
            "{},{:?},{},{}",
            id.0,
            mode,
            b.inner_seconds(),
            a.inner_seconds()
        )?;
    }
    Ok(path)
}

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

use super::trip_problems::{problem_matrix, ProblemType, TripProblemFilter};
use crate::app::{App, Transition};
use crate::common::color_for_mode;
use crate::sandbox::dashboards::DashTab;

pub struct TravelTimes {
    panel: Panel,
}

impl TravelTimes {
    pub fn new_state(ctx: &mut EventCtx, app: &App, filter: Filter) -> Box<dyn State<App>> {
        Box::new(TravelTimes {
            panel: TravelTimes::make_panel(ctx, app, filter),
        })
    }

    fn make_panel(ctx: &mut EventCtx, app: &App, filter: Filter) -> Panel {
        let mut filters = vec!["Filters".text_widget(ctx)];
        for mode in TripMode::all() {
            filters.push(Toggle::colored_checkbox(
                ctx,
                mode.ongoing_verb(),
                color_for_mode(app, mode),
                filter.modes.contains(&mode),
            ));
        }

        filters.push(
            ctx.style()
                .btn_plain
                .text("Export to CSV")
                .build_def(ctx)
                .align_bottom(),
        );

        Panel::new_builder(Widget::col(vec![
            DashTab::TravelTimes.picker(ctx, app),
            Widget::row(vec![
                Widget::col(filters).section(ctx),
                Widget::col(vec![
                    summary_boxes(ctx, app, &filter),
                    Widget::col(vec![
                        Text::from(Line("Travel Times").small_heading()).into_widget(ctx),
                        Widget::row(vec![
                            "filter:".text_widget(ctx).centered_vert(),
                            Widget::dropdown(
                                ctx,
                                "filter",
                                filter.changes_pct,
                                vec![
                                    Choice::new("any change", None),
                                    Choice::new("at least 1% change", Some(0.01)),
                                    Choice::new("at least 10% change", Some(0.1)),
                                    Choice::new("at least 50% change", Some(0.5)),
                                ],
                            ),
                        ])
                        .margin_above(8),
                        Widget::horiz_separator(ctx, 1.0),
                        Widget::row(vec![
                            contingency_table(ctx, app, &filter).bg(ctx.style().section_bg),
                            scatter_plot(ctx, app, &filter)
                                .bg(ctx.style().section_bg)
                                .margin_left(32),
                        ]),
                    ])
                    .section(ctx)
                    .evenly_spaced(),
                    Widget::row(vec![
                        Widget::col(vec![
                            Text::from(Line("Intersection Delays").small_heading())
                                .into_widget(ctx),
                            Toggle::checkbox(
                                ctx,
                                "include trips without any changes",
                                None,
                                filter.include_no_changes(),
                            ),
                        ]),
                        problem_matrix(
                            ctx,
                            app,
                            &filter.trip_problems(app, ProblemType::IntersectionDelay),
                        )
                        .margin_left(32),
                    ])
                    .section(ctx),
                ]),
            ]),
        ]))
        .exact_size_percent(90, 90)
        .build(ctx)
    }
}

impl State<App> for TravelTimes {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Export to CSV" => {
                    return Transition::Push(match export_times(app) {
                        Ok(path) => PopupMsg::new_state(
                            ctx,
                            "Data exported",
                            vec![format!("Data exported to {}", path)],
                        ),
                        Err(err) => {
                            PopupMsg::new_state(ctx, "Export failed", vec![err.to_string()])
                        }
                    });
                }
                "close" => Transition::Pop,
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                if let Some(t) = DashTab::TravelTimes.transition(ctx, app, &self.panel) {
                    return t;
                }

                let mut filter = Filter {
                    changes_pct: self.panel.dropdown_value("filter"),
                    modes: BTreeSet::new(),
                    include_no_changes: self.panel.is_checked("include trips without any changes"),
                };
                for m in TripMode::all() {
                    if self.panel.is_checked(m.ongoing_verb()) {
                        filter.modes.insert(m);
                    }
                }
                let mut new_panel = TravelTimes::make_panel(ctx, app, filter);
                new_panel.restore(ctx, &self.panel);
                self.panel = new_panel;
                Transition::Keep
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
            Line(format!(
                "Saved {} in total",
                sum_faster.to_rounded_string(1)
            ))
            .small(),
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
            Line(format!("Lost {} in total", sum_slower.to_rounded_string(1))).small(),
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
        Line("Trip time before vs. after")
            .small_heading()
            .into_widget(ctx),
        CompareTimes::new_widget(
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
}

fn contingency_table(ctx: &mut EventCtx, app: &App, filter: &Filter) -> Widget {
    let total_width = 500.0;
    let total_height = 300.0;

    let points = filter.get_trips(app);
    if points.is_empty() {
        return Widget::nothing();
    }

    // bucket by trip duration _before_ changes
    let duration_buckets = vec![
        Duration::ZERO,
        Duration::minutes(5),
        Duration::minutes(15),
        Duration::minutes(30),
        Duration::hours(1),
    ];
    let num_buckets = duration_buckets.len();

    let mut batch = GeomBatch::new();
    batch.autocrop_dims = false;

    // Draw the X axis
    let text_height = ctx.default_line_height();
    let text_v_padding = 12.0;
    let x_axis_height = text_height + text_v_padding;
    let line_thickness = Distance::meters(1.5);
    for (idx, mins) in duration_buckets.iter().skip(1).enumerate() {
        let x = (idx as f64 + 1.0) / (num_buckets as f64) * total_width;
        let y = total_height / 2.0;

        {
            let bottom_of_top_bar = (total_height - x_axis_height) / 2.0;
            let line_top = bottom_of_top_bar;
            let line_bottom = bottom_of_top_bar + text_v_padding / 2.0 + 2.0;
            batch.push(
                ctx.style().text_secondary_color.shade(0.2),
                geom::Line::new(Pt2D::new(x, line_top), Pt2D::new(x, line_bottom))
                    .unwrap()
                    .make_polygons(line_thickness),
            );
        }
        {
            let top_of_bottom_bar = (total_height - x_axis_height) / 2.0 + x_axis_height;
            let line_bottom = top_of_bottom_bar;
            let line_top = line_bottom - text_v_padding / 2.0 - 2.0;
            batch.push(
                ctx.style().text_secondary_color.shade(0.2),
                geom::Line::new(Pt2D::new(x, line_top), Pt2D::new(x, line_bottom))
                    .unwrap()
                    .make_polygons(line_thickness),
            );
        }
        batch.append(
            Text::from(Line(mins.to_string()).secondary())
                .render(ctx)
                .centered_on(Pt2D::new(x, y - 4.0)),
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

    #[derive(Clone)]
    struct Changes {
        trip_count: usize,
        accumulated_duration: Duration,
    }

    // Now measure savings and losses per bucket.
    let mut savings_per_bucket = vec![
        Changes {
            trip_count: 0,
            accumulated_duration: Duration::ZERO
        };
        num_buckets
    ];
    let mut losses_per_bucket = vec![
        Changes {
            trip_count: 0,
            accumulated_duration: Duration::ZERO
        };
        num_buckets
    ];

    for (b, a) in points {
        // bucket by trip duration _before_ changes
        let idx = duration_buckets
            .iter()
            .position(|min| *min > b)
            .unwrap_or_else(|| duration_buckets.len())
            - 1;
        match a.cmp(&b) {
            std::cmp::Ordering::Greater => {
                losses_per_bucket[idx].accumulated_duration += a - b;
                losses_per_bucket[idx].trip_count += 1;
            }
            std::cmp::Ordering::Less => {
                savings_per_bucket[idx].accumulated_duration += b - a;
                savings_per_bucket[idx].trip_count += 1;
            }
            std::cmp::Ordering::Equal => {}
        }
    }
    let max_y = losses_per_bucket
        .iter()
        .chain(savings_per_bucket.iter())
        .map(|c| c.accumulated_duration.abs())
        .max()
        .unwrap();

    let intervals = max_y.make_intervals_for_max(2);

    // Draw the bars!
    let bar_width = total_width / (num_buckets as f64);
    let max_bar_height = (total_height - x_axis_height) / 2.0;
    let min_bar_height = 8.0;
    let mut bar_outlines = Vec::new();
    let mut tooltips = Vec::new();
    let mut x1 = 0.0;
    for (
        idx,
        (
            Changes {
                accumulated_duration: total_savings,
                trip_count: num_savings,
            },
            Changes {
                accumulated_duration: total_loss,
                trip_count: num_loss,
            },
        ),
    ) in savings_per_bucket
        .into_iter()
        .zip(losses_per_bucket.into_iter())
        .enumerate()
    {
        if num_savings > 0 {
            let height = ((total_savings / intervals.0) * max_bar_height).max(min_bar_height);
            let rect = Polygon::rectangle(bar_width, height).translate(x1, max_bar_height - height);
            if let Ok(o) = rect.to_outline(line_thickness) {
                bar_outlines.push(o);
            }
            batch.push(Color::GREEN, rect.clone());
            tooltips.push((
                rect,
                Text::from_multiline(vec![
                    Line(match idx {
                        0 => format!(
                            "{} trips shorter than {}",
                            prettyprint_usize(num_savings),
                            duration_buckets[idx + 1]
                        ),
                        i if i + 1 == duration_buckets.len() => format!(
                            "{} trips longer than {}",
                            prettyprint_usize(num_savings),
                            duration_buckets[idx]
                        ),
                        _ => format!(
                            "{} trips between {} and {}",
                            prettyprint_usize(num_savings),
                            duration_buckets[idx],
                            duration_buckets[idx + 1]
                        ),
                    }),
                    Line(format!(
                        "Saved {} in total",
                        total_savings.to_rounded_string(1)
                    ))
                    .fg(Color::hex("#72CE36")),
                ]),
            ));
        }
        if num_loss > 0 {
            let height = ((total_loss / intervals.0) * max_bar_height).max(min_bar_height);
            let rect =
                Polygon::rectangle(bar_width, height).translate(x1, total_height - max_bar_height);
            if let Ok(o) = rect.to_outline(line_thickness) {
                bar_outlines.push(o);
            }
            batch.push(Color::RED, rect.clone());
            tooltips.push((
                rect,
                Text::from_multiline(vec![
                    Line(match idx {
                        0 => format!(
                            "{} trips shorter than {}",
                            prettyprint_usize(num_loss),
                            duration_buckets[idx + 1]
                        ),
                        i if i + 1 == duration_buckets.len() => format!(
                            "{} trips longer than {}",
                            prettyprint_usize(num_loss),
                            duration_buckets[idx]
                        ),
                        _ => format!(
                            "{} trips between {} and {}",
                            prettyprint_usize(num_loss),
                            duration_buckets[idx],
                            duration_buckets[idx + 1]
                        ),
                    }),
                    Line(format!("Lost {} in total", total_loss.to_rounded_string(1)))
                        .fg(Color::hex("#EB3223")),
                ]),
            ));
        }
        x1 += bar_width;
    }
    // Draw the y-axis
    let mut y_axis_ticks = GeomBatch::new();
    let mut y_axis_labels = GeomBatch::new();
    {
        let line_length = 8.0;
        let line_thickness = 2.0;

        intervals.1[1..]
            .iter()
            .map(|interval| {
                // positive ticks
                let y =
                    max_bar_height * (1.0 - interval.inner_seconds() / intervals.0.inner_seconds());
                (interval, y)
            })
            .chain(
                // negative ticks
                intervals.1[1..].iter().map(|interval| {
                    let y = total_height
                        - max_bar_height
                            * (1.0 - interval.abs().inner_seconds() / intervals.0.inner_seconds());
                    (interval, y)
                }),
            )
            .for_each(|(interval, y)| {
                let start = Pt2D::new(0.0, y);
                let line: geom::Line =
                    geom::Line::new(start, start.offset(line_length, 0.0)).unwrap();
                let poly = line.make_polygons(Distance::meters(line_thickness));
                y_axis_ticks.push(ctx.style().text_secondary_color, poly);

                let text = Text::from(Line(interval.abs().to_rounded_string(0)).secondary())
                    .render(ctx)
                    .centered_on(start.offset(0.0, -4.0));
                y_axis_labels.append(text);
            });
    }
    y_axis_labels.autocrop_dims = true;
    y_axis_labels = y_axis_labels.autocrop();

    batch.extend(Color::BLACK, bar_outlines);

    Widget::col(vec![
        Text::from_multiline(vec![
            Line("Aggregate difference by trip duration").small_heading(),
            Line(format!(
                "Grouped by the duration of the trip before\n\"{}\" changes.",
                app.primary.map.get_edits().edits_name
            )),
        ])
        .into_widget(ctx)
        .container(),
        Line("Total Time Saved (faster)")
            .secondary()
            .into_widget(ctx)
            .centered_horiz(),
        Widget::custom_row(vec![
            y_axis_labels
                .into_widget(ctx)
                .margin_right(8)
                .centered_vert(),
            y_axis_ticks
                .into_widget(ctx)
                .margin_right(8)
                .centered_vert(),
            DrawWithTooltips::new_widget(ctx, batch, tooltips, Box::new(|_| GeomBatch::new())),
        ])
        .centered_horiz(),
        Line("Total Time Lost (slower)")
            .secondary()
            .into_widget(ctx)
            .centered_horiz(),
    ])
    .centered()
}

pub struct Filter {
    changes_pct: Option<f64>,
    modes: BTreeSet<TripMode>,
    include_no_changes: bool,
}

impl TripProblemFilter for Filter {
    fn includes_mode(&self, mode: &TripMode) -> bool {
        self.modes.contains(mode)
    }

    fn include_no_changes(&self) -> bool {
        self.include_no_changes
    }
}

impl Filter {
    pub fn new() -> Filter {
        Filter {
            changes_pct: None,
            modes: TripMode::all().into_iter().collect(),
            include_no_changes: false,
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

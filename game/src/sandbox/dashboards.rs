use crate::app::App;
use crate::common::Tab;
use crate::game::{msg, State, Transition};
use crate::sandbox::trip_table::TripTable;
use crate::sandbox::SandboxMode;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Choice, Color, Composite, DrawWithTooltips, EventCtx, GeomBatch, GfxCtx, Key,
    Line, LinePlot, Outcome, PlotOptions, ScatterPlot, Series, Text, TextExt, Widget,
};
use geom::{Distance, Duration, Polygon, Pt2D, Time};

// Oh the dashboards melted, but we still had the radio
#[derive(PartialEq)]
pub enum DashTab {
    TripTable,
    TripSummaries,
    BusRoutes,
}

impl DashTab {
    pub fn picker(self, ctx: &EventCtx) -> Widget {
        let mut row = Vec::new();
        for (name, tab) in vec![
            ("trip table", DashTab::TripTable),
            ("trip summaries", DashTab::TripSummaries),
            ("bus routes", DashTab::BusRoutes),
        ] {
            if self == tab {
                row.push(Btn::text_bg2(name).inactive(ctx));
            } else {
                row.push(Btn::text_bg2(name).build_def(ctx, None));
            }
        }
        Widget::row(vec![
            // TODO Centered, but actually, we need to set the padding of each button to divide the
            // available space evenly. Fancy fill rules... hmmm.
            Widget::row(row).bg(Color::WHITE).margin_vert(16),
            Btn::plaintext("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ])
    }

    pub fn transition(self, ctx: &mut EventCtx, app: &App, action: &str) -> Transition {
        match action {
            "close" => Transition::Pop,
            "trip table" => Transition::Replace(TripTable::new(ctx, app)),
            "trip summaries" => Transition::Replace(TripSummaries::new(ctx, app, None)),
            "bus routes" => Transition::Replace(BusRoutes::new(ctx, app)),
            _ => unreachable!(),
        }
    }
}

struct TripSummaries {
    composite: Composite,
    filter_changes_pct: Option<f64>,
}

impl TripSummaries {
    fn new(ctx: &mut EventCtx, app: &App, filter_changes_pct: Option<f64>) -> Box<dyn State> {
        let mut active_agents = vec![Series {
            label: "After changes".to_string(),
            color: Color::RED,
            pts: app
                .primary
                .sim
                .get_analytics()
                .active_agents(app.primary.sim.time()),
        }];
        if app.has_prebaked().is_some() {
            active_agents.push(Series {
                label: "Before changes".to_string(),
                color: Color::BLUE.alpha(0.5),
                pts: app.prebaked().active_agents(Time::END_OF_DAY),
            });
        }

        Box::new(TripSummaries {
            filter_changes_pct,
            composite: Composite::new(
                Widget::col(vec![
                    DashTab::TripSummaries.picker(ctx),
                    summary(ctx, app).margin_below(10),
                    Widget::row(vec![
                        "Filter:".draw_text(ctx).margin_right(5),
                        Widget::dropdown(
                            ctx,
                            "filter",
                            filter_changes_pct,
                            vec![
                                Choice::new("all trips", None),
                                Choice::new("at least 1% change", Some(0.01)),
                                Choice::new("at least 10% change", Some(0.1)),
                                Choice::new("at least 50% change", Some(0.5)),
                            ],
                        ),
                    ])
                    .centered_horiz()
                    .margin_below(10),
                    Widget::row(vec![
                        contingency_table(ctx, app, filter_changes_pct)
                            .centered_vert()
                            .margin_right(20),
                        scatter_plot(ctx, app, filter_changes_pct),
                    ])
                    .evenly_spaced(),
                    Line("Active agents").small_heading().draw(ctx),
                    LinePlot::new(ctx, "active agents", active_agents, PlotOptions::new()),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .max_size_percent(90, 90)
            .build(ctx),
        })
    }
}

impl State for TripSummaries {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => DashTab::TripSummaries.transition(ctx, app, &x),
            None => {
                let filter = self.composite.dropdown_value("filter");
                if filter != self.filter_changes_pct {
                    Transition::Replace(TripSummaries::new(ctx, app, filter))
                } else {
                    Transition::Keep
                }
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

struct BusRoutes {
    composite: Composite,
}

impl BusRoutes {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let mut routes: Vec<String> = app
            .primary
            .map
            .get_all_bus_routes()
            .iter()
            .map(|r| r.name.clone())
            .collect();
        // TODO Sort first by length, then lexicographically
        routes.sort();

        let mut col = vec![
            DashTab::BusRoutes.picker(ctx),
            Line("Bus routes").small_heading().draw(ctx),
        ];
        for r in routes {
            col.push(Btn::text_fg(r).build_def(ctx, None).margin(5));
        }

        Box::new(BusRoutes {
            composite: Composite::new(Widget::col(col).bg(app.cs.panel_bg).padding(10))
                .max_size_percent(90, 90)
                .build(ctx),
        })
    }
}

impl State for BusRoutes {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                if let Some(r) = app.primary.map.get_bus_route(&x) {
                    let buses = app.primary.sim.status_of_buses(r.id);
                    if buses.is_empty() {
                        Transition::Push(msg(
                            "No buses running",
                            vec![format!("Sorry, no buses for route {} running", r.name)],
                        ))
                    } else {
                        Transition::PopWithData(Box::new(move |state, app, ctx| {
                            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                // Arbitrarily use the first one
                                Tab::BusStatus(buses[0].0),
                                &mut actions,
                            );
                        }))
                    }
                } else {
                    DashTab::BusRoutes.transition(ctx, app, &x)
                }
            }
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

fn summary(ctx: &mut EventCtx, app: &App) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let mut num_same = 0;
    let mut num_faster = 0;
    let mut num_slower = 0;
    let mut sum_faster = Duration::ZERO;
    let mut sum_slower = Duration::ZERO;
    for (b, a) in app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked())
    {
        if a == b {
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

fn scatter_plot(ctx: &mut EventCtx, app: &App, filter_changes_pct: Option<f64>) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let mut points = app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked());
    if let Some(pct) = filter_changes_pct {
        points.retain(|(a, b)| pct_diff(*a, *b) > pct);
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

fn contingency_table(ctx: &mut EventCtx, app: &App, filter_changes_pct: Option<f64>) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let total_width = 500.0;
    let total_height = 300.0;

    let mut points = app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked());
    if let Some(pct) = filter_changes_pct {
        points.retain(|(a, b)| pct_diff(*a, *b) > pct);
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
    for (b, a) in app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked())
    {
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

fn pct_diff(a: Duration, b: Duration) -> f64 {
    if a >= b {
        (a / b) - 1.0
    } else {
        (b / a) - 1.0
    }
}

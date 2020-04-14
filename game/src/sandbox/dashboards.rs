use crate::app::App;
use crate::common::Tab;
use crate::game::{msg, State, Transition};
use crate::sandbox::histogram::Histogram;
use crate::sandbox::trip_table::TripTable;
use crate::sandbox::SandboxMode;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, Key, Line, LinePlot, Outcome,
    PlotOptions, Series, Text, TextExt, Widget,
};
use geom::{Angle, Circle, Distance, Duration, Polygon, Pt2D, Time};

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
            "trip summaries" => Transition::Replace(TripSummaries::new(ctx, app)),
            "bus routes" => Transition::Replace(BusRoutes::new(ctx, app)),
            _ => unreachable!(),
        }
    }
}

struct TripSummaries {
    composite: Composite,
}

impl TripSummaries {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
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
            composite: Composite::new(
                Widget::col(vec![
                    DashTab::TripSummaries.picker(ctx),
                    scatter_plot(ctx, app),
                    summary_absolute(ctx, app),
                    summary_normalized(ctx, app),
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
            None => Transition::Keep,
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

fn summary_absolute(ctx: &mut EventCtx, app: &App) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let mut num_same = 0;
    let mut faster = Vec::new();
    let mut slower = Vec::new();
    let mut sum_faster = Duration::ZERO;
    let mut sum_slower = Duration::ZERO;
    for (a, b) in app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked())
    {
        if a == b {
            num_same += 1;
        } else if a < b {
            faster.push(b - a);
            sum_faster += b - a;
        } else {
            slower.push(a - b);
            sum_slower += a - b;
        }
    }

    // TODO Outliers are heavy -- median instead of average?
    // TODO Filters for mode
    Widget::col(vec![
        Line("Are finished trips faster or slower?")
            .draw(ctx)
            .margin_below(5),
        Widget::row(vec![
            Widget::col(vec![
                Text::from_multiline(vec![
                    Line(format!("{} trips faster", prettyprint_usize(faster.len()))),
                    Line(format!("{} total time saved", sum_faster)),
                    Line(format!(
                        "Average {} per faster trip",
                        if faster.is_empty() {
                            Duration::ZERO
                        } else {
                            sum_faster / (faster.len() as f64)
                        }
                    )),
                ])
                .draw(ctx)
                .margin_below(5),
                Histogram::new(ctx, Color::GREEN, faster),
            ])
            .outline(2.0, Color::WHITE)
            .padding(10),
            Line(format!("{} trips unchanged", prettyprint_usize(num_same)))
                .draw(ctx)
                .centered_vert(),
            Widget::col(vec![
                Text::from_multiline(vec![
                    Line(format!("{} trips slower", prettyprint_usize(slower.len()))),
                    Line(format!("{} total time lost", sum_slower)),
                    Line(format!(
                        "Average {} per slower trip",
                        if slower.is_empty() {
                            Duration::ZERO
                        } else {
                            sum_slower / (slower.len() as f64)
                        }
                    )),
                ])
                .draw(ctx)
                .margin_below(5),
                Histogram::new(ctx, Color::RED, slower),
            ])
            .outline(2.0, Color::WHITE)
            .padding(10),
        ])
        .evenly_spaced(),
    ])
}

fn summary_normalized(ctx: &mut EventCtx, app: &App) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let mut num_same = 0;
    let mut faster = Vec::new();
    let mut slower = Vec::new();
    for (a, b) in app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked())
    {
        if a == b {
            num_same += 1;
        } else if a < b {
            // TODO Hack: map percentages in [0.0, 100.0] to seconds
            faster.push(Duration::seconds((1.0 - (a / b)) * 100.0));
        } else {
            slower.push(Duration::seconds(((a / b) - 1.0) * 100.0));
        }
    }

    // TODO Show average?
    // TODO Filters for mode
    // TODO Is summing percentages meaningful?
    Widget::col(vec![
        Line("Are finished trips faster or slower? (normalized to original trip time)")
            .draw(ctx)
            .margin_below(5),
        Widget::row(vec![
            Widget::col(vec![
                format!("{} trips faster", prettyprint_usize(faster.len()))
                    .draw_text(ctx)
                    .margin_below(5),
                Histogram::new(ctx, Color::GREEN, faster),
            ])
            .outline(2.0, Color::WHITE)
            .padding(10),
            Line(format!("{} trips unchanged", prettyprint_usize(num_same)))
                .draw(ctx)
                .centered_vert(),
            Widget::col(vec![
                format!("{} trips slower", prettyprint_usize(slower.len()))
                    .draw_text(ctx)
                    .margin_below(5),
                Histogram::new(ctx, Color::RED, slower),
            ])
            .outline(2.0, Color::WHITE)
            .padding(10),
        ])
        .evenly_spaced(),
    ])
}

fn scatter_plot(ctx: &mut EventCtx, app: &App) -> Widget {
    if app.has_prebaked().is_none() {
        return Widget::nothing();
    }

    let points = app
        .primary
        .sim
        .get_analytics()
        .both_finished_trips(app.primary.sim.time(), app.prebaked());
    if points.is_empty() {
        return Widget::nothing();
    }

    let actual_max = *points.iter().map(|(a, b)| a.max(b)).max().unwrap();
    let (max, labels) = make_intervals(actual_max, 5);

    // We want a nice square so the scales match up.
    let width = 500.0;
    let height = width;

    let mut batch = GeomBatch::new();
    batch.autocrop_dims = false;
    batch.push(Color::BLACK, Polygon::rectangle(width, width));

    let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(4.0)).to_polygon();
    for (a, b) in points {
        let pt = Pt2D::new((a / max) * width, (1.0 - (b / max)) * height);
        // TODO Could color circles by mode
        let color = if a == b {
            Color::YELLOW.alpha(0.5)
        } else if a < b {
            Color::GREEN.alpha(0.9)
        } else {
            Color::RED.alpha(0.9)
        };
        batch.push(color, circle.translate(pt.x(), pt.y()));
    }
    let plot = Widget::draw_batch(ctx, batch);

    let y_axis = Widget::col(
        labels
            .iter()
            .rev()
            .map(|x| Line(x.to_string()).small().draw(ctx))
            .collect(),
    )
    .evenly_spaced();
    let y_label = {
        let mut label = GeomBatch::new();
        for (color, poly) in Text::from(Line("Current trip time (minutes)"))
            .render_ctx(ctx)
            .consume()
        {
            label.fancy_push(color, poly.rotate(Angle::new_degs(90.0)));
        }
        Widget::draw_batch(ctx, label.autocrop()).centered_vert()
    };

    let x_axis = Widget::row(
        labels
            .iter()
            .map(|x| Line(x.to_string()).small().draw(ctx))
            .collect(),
    )
    .evenly_spaced();
    let x_label = Line("Original trip time (minutes)")
        .draw(ctx)
        .centered_horiz();

    // It's a bit of work to make both the x and y axis line up with the plot. :)
    let plot_width = plot.get_width_for_forcing();
    Widget::row(vec![Widget::col(vec![
        Widget::row(vec![y_label, y_axis, plot]),
        Widget::col(vec![x_axis, x_label])
            .force_width(plot_width)
            .align_right(),
    ])])
}

// TODO Do something fancier? http://vis.stanford.edu/papers/tick-labels
fn make_intervals(actual_max: Duration, num_labels: usize) -> (Duration, Vec<usize>) {
    // Example: 43 minutes, max 5 labels... raw_mins_per_interval is 8.6
    let raw_mins_per_interval =
        (actual_max.num_minutes_rounded_down() as f64) / (num_labels as f64);
    // So then this rounded up to 10 minutes
    let mins_per_interval = Duration::seconds(60.0 * raw_mins_per_interval)
        .round_up(Duration::minutes(5))
        .num_minutes_rounded_down();

    (
        actual_max.round_up(Duration::minutes(mins_per_interval)),
        (0..=num_labels).map(|i| i * mins_per_interval).collect(),
    )
}

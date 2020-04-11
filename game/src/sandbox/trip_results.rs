use crate::app::App;
use crate::game::{State, Transition};
use crate::helpers::cmp_duration_shorter;
use crate::info::Tab;
use crate::sandbox::SandboxMode;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, EventCtx, GfxCtx, Histogram, Key, Line, Outcome, Text,
    TextExt, Widget,
};
use geom::{Duration, Time};
use maplit::btreeset;
use sim::{TripID, TripMode};

// TODO Hover over a trip to preview its route on the map

pub struct TripResults {
    composite: Composite,
    sort_by: SortBy,
    descending: bool,
}

// TODO Is there a heterogenously typed table crate somewhere?
#[derive(Clone, Copy, PartialEq)]
enum SortBy {
    Departure,
    Duration,
    RelativeDuration,
    PercentChangeDuration,
    PercentWaiting,
}

impl TripResults {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(TripResults {
            composite: make(ctx, app, SortBy::PercentWaiting, true),
            sort_by: SortBy::PercentWaiting,
            descending: true,
        })
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = make(ctx, app, self.sort_by, self.descending);
        new.restore(ctx, &self.composite);
        self.composite = new;
    }
}

impl State for TripResults {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Departure" => {
                    self.sort_by = SortBy::Departure;
                    self.recalc(ctx, app);
                }
                "Duration" => {
                    self.sort_by = SortBy::Duration;
                    self.recalc(ctx, app);
                }
                "Comparison" => {
                    self.sort_by = SortBy::RelativeDuration;
                    self.recalc(ctx, app);
                }
                "Normalized" => {
                    self.sort_by = SortBy::PercentChangeDuration;
                    self.recalc(ctx, app);
                }
                "Percent waiting" => {
                    self.sort_by = SortBy::PercentWaiting;
                    self.recalc(ctx, app);
                }
                x => {
                    let trip = TripID(x.parse::<usize>().unwrap());
                    let person = app.primary.sim.trip_to_person(trip);
                    return Transition::PopWithData(Box::new(move |state, app, ctx| {
                        let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                        let mut actions = sandbox.contextual_actions();
                        sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                            ctx,
                            app,
                            Tab::PersonTrips(person, btreeset! { trip }),
                            &mut actions,
                        );
                    }));
                }
            },
            None => {}
        };
        let descending = self.composite.is_checked("Descending");
        if self.descending != descending {
            self.descending = descending;
            self.recalc(ctx, app);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

struct Entry {
    trip: TripID,
    mode: TripMode,
    departure: Time,
    duration: Duration,
    baseline_duration: Duration,
    waiting: Duration,
    percent_waiting: usize,
}

fn make(ctx: &mut EventCtx, app: &App, sort: SortBy, descending: bool) -> Composite {
    let mut data = Vec::new();
    let sim = &app.primary.sim;
    for (_, id, maybe_mode, duration) in &sim.get_analytics().finished_trips {
        let mode = if let Some(m) = maybe_mode {
            *m
        } else {
            continue;
        };
        let (_, waiting) = sim.finished_trip_time(*id).unwrap();
        let (departure, _, _, _) = sim.trip_info(*id);
        let baseline_duration = if app.has_prebaked().is_some() {
            app.prebaked().finished_trip_time(*id).unwrap()
        } else {
            Duration::ZERO
        };

        data.push(Entry {
            trip: *id,
            mode,
            departure,
            duration: *duration,
            baseline_duration,
            waiting,
            percent_waiting: (100.0 * waiting / *duration) as usize,
        });
    }

    match sort {
        SortBy::Departure => data.sort_by_key(|x| x.departure),
        SortBy::Duration => data.sort_by_key(|x| x.duration),
        SortBy::RelativeDuration => data.sort_by_key(|x| x.duration - x.baseline_duration),
        SortBy::PercentChangeDuration => {
            data.sort_by_key(|x| (100.0 * (x.duration / x.baseline_duration)) as isize)
        }
        SortBy::PercentWaiting => data.sort_by_key(|x| x.percent_waiting),
    }
    if descending {
        data.reverse();
    }

    // Cheap tabular layout
    // TODO https://stackoverflow.com/questions/48493500/can-flexbox-handle-varying-sizes-of-columns-but-consistent-row-height/48496343#48496343
    // For now, manually tuned margins :(
    let mut id_col = Vec::new();
    let mut mode_col = Text::new();
    let mut departure_col = Text::new();
    let mut duration_col = Text::new();
    let mut relative_duration_col = Text::new();
    let mut percent_change_duration_col = Text::new();
    let mut waiting_col = Text::new();
    let mut pct_waiting_col = Text::new();

    for x in data.into_iter().take(30) {
        id_col.push(Btn::plaintext(x.trip.0.to_string()).build_def(ctx, None));
        mode_col.add(Line(x.mode.ongoing_verb()));
        departure_col.add(Line(x.departure.ampm_tostring()));
        duration_col.add(Line(x.duration.to_string()));
        if app.has_prebaked().is_some() {
            relative_duration_col
                .add_appended(cmp_duration_shorter(x.duration, x.baseline_duration));
            if x.duration == x.baseline_duration {
                percent_change_duration_col.add(Line("same"));
            } else if x.duration < x.baseline_duration {
                percent_change_duration_col.add(Line(format!(
                    "{}% faster",
                    (100.0 * (1.0 - (x.duration / x.baseline_duration))) as usize
                )));
            } else {
                percent_change_duration_col.add(Line(format!(
                    "{}% slower ",
                    (100.0 * ((x.duration / x.baseline_duration) - 1.0)) as usize
                )));
            }
        }
        waiting_col.add(Line(x.waiting.to_string()));
        pct_waiting_col.add(Line(format!("{}%", x.percent_waiting)));
    }

    let mut table = vec![
        (Line("Trip ID").draw(ctx), Widget::col(id_col)),
        (Line("Type").draw(ctx), mode_col.draw(ctx)),
        (
            if sort == SortBy::Departure {
                Btn::text_bg2("Departure").inactive(ctx)
            } else {
                Btn::text_fg("Departure").build_def(ctx, None)
            },
            departure_col.draw(ctx),
        ),
        (
            if sort == SortBy::Duration {
                Btn::text_bg2("Duration").inactive(ctx)
            } else {
                Btn::text_fg("Duration").build_def(ctx, None)
            },
            duration_col.draw(ctx),
        ),
    ];
    if app.has_prebaked().is_some() {
        table.push((
            if sort == SortBy::RelativeDuration {
                Btn::text_bg2("Comparison").inactive(ctx)
            } else {
                Btn::text_fg("Comparison").build_def(ctx, None)
            },
            relative_duration_col.draw(ctx),
        ));
        table.push((
            if sort == SortBy::PercentChangeDuration {
                Btn::text_bg2("Normalized ").inactive(ctx)
            } else {
                Btn::text_fg("Normalized").build_def(ctx, None)
            },
            percent_change_duration_col.draw(ctx),
        ));
    }
    table.push((Line("Time spent waiting").draw(ctx), waiting_col.draw(ctx)));
    table.push((
        if sort == SortBy::PercentWaiting {
            Btn::text_bg2("Percent waiting").inactive(ctx)
        } else {
            Btn::text_fg("Percent waiting").build_def(ctx, None)
        },
        pct_waiting_col.draw(ctx),
    ));

    let mut header_row = Vec::new();
    let mut values_row = Vec::new();
    for (header, values) in table {
        let width = header
            .get_width_for_forcing()
            .max(values.get_width_for_forcing());
        header_row.push(header.force_width(width).margin_right(10));
        values_row.push(
            Widget::col(vec![values])
                .force_width(width)
                .margin_right(10),
        );
    }

    Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Line("Finished trips").small_heading().draw(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", hotkey(Key::Escape))
                    .align_right(),
            ]),
            summary_absolute(ctx, app).margin(20),
            summary_normalized(ctx, app).margin(20),
            Line("Click a column to sort by it").small().draw(ctx),
            Checkbox::text(ctx, "Descending", None, descending).margin(10),
            Widget::row(header_row).evenly_spaced(),
            Widget::row(values_row).evenly_spaced(),
        ])
        // TODO Until exact_size_percent supports scrolling, do this hack
        .force_width_pct(ctx, 90)
        .bg(app.cs.panel_bg)
        .padding(10),
    )
    .max_size_percent(90, 90)
    .build(ctx)
}

// TODO Not sure where this should live yet. New ideas for summarizing different trips.
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

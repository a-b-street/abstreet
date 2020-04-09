use crate::app::App;
use crate::game::{State, Transition};
use crate::helpers::cmp_duration_shorter;
use crate::info::Tab;
use crate::sandbox::SandboxMode;
use ezgui::{hotkey, Btn, Checkbox, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Text, Widget};
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
        self.composite = make(ctx, app, self.sort_by, self.descending);
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
                "Comparison with baseline" => {
                    self.sort_by = SortBy::RelativeDuration;
                    self.recalc(ctx, app);
                }
                "Percent of trip spent waiting" => {
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
        SortBy::PercentWaiting => data.sort_by_key(|x| x.percent_waiting),
    }
    if descending {
        data.reverse();
    }

    // Cheap tabular layout
    // TODO https://stackoverflow.com/questions/48493500/can-flexbox-handle-varying-sizes-of-columns-but-consistent-row-height/48496343#48496343
    // For now, manually tuned margins :(
    let mut col1 = Vec::new();
    let mut col2 = Text::new();
    let mut col3 = Text::new();
    let mut col4 = Text::new();
    let mut maybe_col5 = Text::new();
    let mut col6 = Text::new();
    let mut col7 = Text::new();

    for x in data.into_iter().take(30) {
        col1.push(Btn::plaintext(x.trip.0.to_string()).build_def(ctx, None));
        col2.add(Line(x.mode.ongoing_verb()));
        col3.add(Line(x.departure.ampm_tostring()));
        col4.add(Line(x.duration.to_string()));
        if app.has_prebaked().is_some() {
            maybe_col5.add_appended(cmp_duration_shorter(x.duration, x.baseline_duration));
        }
        col6.add(Line(x.waiting.to_string()));
        col7.add(Line(format!("{}%", x.percent_waiting)));
    }

    Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Line("Finished trips").small_heading().draw(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", hotkey(Key::Escape))
                    .align_right(),
            ]),
            Line("Click a column to sort by it").small().draw(ctx),
            Checkbox::text(ctx, "Descending", None, descending).margin(10),
            // TODO The column names aren't lined up at all
            Widget::row(vec![
                Line("Trip ID").draw(ctx).margin_right(10),
                Line("Type").draw(ctx).margin_right(10),
                if sort == SortBy::Departure {
                    Btn::text_bg2("Departure").inactive(ctx)
                } else {
                    Btn::text_fg("Departure").build_def(ctx, None)
                }
                .margin_right(10),
                if sort == SortBy::Duration {
                    Btn::text_bg2("Duration").inactive(ctx)
                } else {
                    Btn::text_fg("Duration").build_def(ctx, None)
                }
                .margin_right(10),
                if app.has_prebaked().is_some() {
                    if sort == SortBy::RelativeDuration {
                        Btn::text_bg2("Comparison with baseline").inactive(ctx)
                    } else {
                        Btn::text_fg("Comparison with baseline").build_def(ctx, None)
                    }
                    .margin_right(10)
                } else {
                    Widget::nothing()
                },
                Line("Time spent waiting").draw(ctx).margin_right(10),
                if sort == SortBy::PercentWaiting {
                    Btn::text_bg2("Percent of trip spent waiting").inactive(ctx)
                } else {
                    Btn::text_fg("Percent of trip spent waiting").build_def(ctx, None)
                }
                .margin_right(10),
            ]),
            Widget::row(vec![
                Widget::col(col1).margin_right(10),
                col2.draw(ctx).margin_right(10),
                col3.draw(ctx).margin_right(10),
                col4.draw(ctx).margin_right(10),
                if app.has_prebaked().is_some() {
                    maybe_col5.draw(ctx).margin_right(10)
                } else {
                    Widget::nothing()
                },
                col6.draw(ctx).margin_right(10),
                col7.draw(ctx).margin_right(10),
            ])
            .evenly_spaced(),
        ])
        .bg(app.cs.panel_bg)
        .padding(10),
    )
    .max_size_percent(90, 90)
    .build(ctx)
}

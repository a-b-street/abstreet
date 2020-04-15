use crate::app::App;
use crate::game::{State, Transition};
use crate::helpers::cmp_duration_shorter;
use crate::info::Tab;
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;
use ezgui::{Btn, Composite, EventCtx, GfxCtx, Line, Outcome, Text, Widget};
use geom::{Duration, Time};
use maplit::btreemap;
use sim::{TripID, TripMode};

// TODO Hover over a trip to preview its route on the map

pub struct TripTable {
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

impl TripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(TripTable {
            composite: make(ctx, app, SortBy::PercentWaiting, true),
            sort_by: SortBy::PercentWaiting,
            descending: true,
        })
    }

    fn change(&mut self, value: SortBy) {
        if self.sort_by == value {
            self.descending = !self.descending;
        } else {
            self.sort_by = value;
            self.descending = true;
        }
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = make(ctx, app, self.sort_by, self.descending);
        new.restore(ctx, &self.composite);
        self.composite = new;
    }
}

impl State for TripTable {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Departure" => {
                    self.change(SortBy::Departure);
                    self.recalc(ctx, app);
                }
                "Duration" => {
                    self.change(SortBy::Duration);
                    self.recalc(ctx, app);
                }
                "Comparison" => {
                    self.change(SortBy::RelativeDuration);
                    self.recalc(ctx, app);
                }
                "Normalized" => {
                    self.change(SortBy::PercentChangeDuration);
                    self.recalc(ctx, app);
                }
                "Percent waiting" => {
                    self.change(SortBy::PercentWaiting);
                    self.recalc(ctx, app);
                }
                x => {
                    if let Ok(idx) = x.parse::<usize>() {
                        let trip = TripID(idx);
                        let person = app.primary.sim.trip_to_person(trip);
                        return Transition::PopWithData(Box::new(move |state, app, ctx| {
                            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::PersonTrips(person, btreemap! { trip => true }),
                                &mut actions,
                            );
                        }));
                    }
                    return DashTab::TripTable.transition(ctx, app, x);
                }
            },
            None => {}
        };

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
    duration_after: Duration,
    duration_before: Duration,
    waiting: Duration,
    percent_waiting: usize,
}

fn make(ctx: &mut EventCtx, app: &App, sort: SortBy, descending: bool) -> Composite {
    let mut data = Vec::new();
    let sim = &app.primary.sim;
    for (_, id, maybe_mode, duration_after) in &sim.get_analytics().finished_trips {
        let mode = if let Some(m) = maybe_mode {
            *m
        } else {
            continue;
        };
        let (_, waiting) = sim.finished_trip_time(*id).unwrap();
        let (departure, _, _, _) = sim.trip_info(*id);
        let duration_before = if app.has_prebaked().is_some() {
            app.prebaked().finished_trip_time(*id).unwrap()
        } else {
            Duration::ZERO
        };

        data.push(Entry {
            trip: *id,
            mode,
            departure,
            duration_after: *duration_after,
            duration_before,
            waiting,
            percent_waiting: (100.0 * waiting / *duration_after) as usize,
        });
    }

    match sort {
        SortBy::Departure => data.sort_by_key(|x| x.departure),
        SortBy::Duration => data.sort_by_key(|x| x.duration_after),
        SortBy::RelativeDuration => data.sort_by_key(|x| x.duration_after - x.duration_before),
        SortBy::PercentChangeDuration => {
            data.sort_by_key(|x| (100.0 * (x.duration_after / x.duration_before)) as isize)
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
        duration_col.add(Line(x.duration_after.to_string()));
        if app.has_prebaked().is_some() {
            relative_duration_col
                .add_appended(cmp_duration_shorter(x.duration_after, x.duration_before));
            if x.duration_after == x.duration_before {
                percent_change_duration_col.add(Line("same"));
            } else if x.duration_after < x.duration_before {
                percent_change_duration_col.add(Line(format!(
                    "{}% faster",
                    (100.0 * (1.0 - (x.duration_after / x.duration_before))) as usize
                )));
            } else {
                percent_change_duration_col.add(Line(format!(
                    "{}% slower ",
                    (100.0 * ((x.duration_after / x.duration_before) - 1.0)) as usize
                )));
            }
        }
        waiting_col.add(Line(x.waiting.to_string()));
        pct_waiting_col.add(Line(format!("{}%", x.percent_waiting)));
    }

    let btn = |value, name| {
        if sort == value {
            Btn::text_bg2(format!("{} {}", name, if descending { "↓" } else { "↑" }))
                .build(ctx, name, None)
        } else {
            Btn::text_bg2(name).build_def(ctx, None)
        }
    };

    let mut table = vec![
        (Line("Trip ID").draw(ctx), Widget::col(id_col)),
        (Line("Type").draw(ctx), mode_col.draw(ctx)),
        (btn(SortBy::Departure, "Departure"), departure_col.draw(ctx)),
        (btn(SortBy::Duration, "Duration"), duration_col.draw(ctx)),
    ];
    if app.has_prebaked().is_some() {
        table.push((
            btn(SortBy::RelativeDuration, "Comparison"),
            relative_duration_col.draw(ctx),
        ));
        table.push((
            btn(SortBy::PercentChangeDuration, "Normalized"),
            percent_change_duration_col.draw(ctx),
        ));
    }
    table.push((Line("Time spent waiting").draw(ctx), waiting_col.draw(ctx)));
    table.push((
        btn(SortBy::PercentWaiting, "Percent waiting"),
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
            DashTab::TripTable.picker(ctx),
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

use crate::app::App;
use crate::game::{State, Transition};
use crate::helpers::cmp_duration_shorter;
use crate::info::Tab;
use crate::sandbox::SandboxMode;
use ezgui::{hotkey, Btn, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Text, Widget};
use maplit::btreeset;
use sim::TripID;

// TODO Hover over a trip to preview its route on the map

pub struct TripResults {
    composite: Composite,
}

// TODO Is there a heterogenously typed table crate somewhere?
#[derive(PartialEq)]
enum SortBy {
    Departure,
    Duration,
    PercentWaiting,
}

impl TripResults {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(TripResults {
            composite: make(ctx, app, SortBy::PercentWaiting),
        })
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
                    self.composite = make(ctx, app, SortBy::Departure);
                }
                "Duration" => {
                    self.composite = make(ctx, app, SortBy::Duration);
                }
                "Percent of trip spent waiting" => {
                    self.composite = make(ctx, app, SortBy::PercentWaiting);
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
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

fn make(ctx: &mut EventCtx, app: &App, sort: SortBy) -> Composite {
    let mut data = Vec::new();
    let sim = &app.primary.sim;
    for (_, id, maybe_mode, duration) in &sim.get_analytics().finished_trips {
        let mode = if let Some(m) = maybe_mode {
            *m
        } else {
            continue;
        };
        let (_, blocked) = sim.finished_trip_time(*id).unwrap();
        let (start_time, _, _, _) = sim.trip_info(*id);
        let comparison = if app.has_prebaked().is_some() {
            cmp_duration_shorter(*duration, app.prebaked().finished_trip_time(*id).unwrap())
        } else {
            vec![Line("n/a")]
        };

        data.push((
            *id,
            mode,
            start_time,
            *duration,
            comparison,
            blocked,
            (100.0 * blocked / *duration) as usize,
        ));
    }

    match sort {
        SortBy::Departure => data.sort_by_key(|(_, _, t, _, _, _, _)| *t),
        SortBy::Duration => data.sort_by_key(|(_, _, _, dt, _, _, _)| *dt),
        SortBy::PercentWaiting => data.sort_by_key(|(_, _, _, _, _, _, pct)| *pct),
    }
    // Descending...
    data.reverse();

    // Cheap tabular layout
    // TODO https://stackoverflow.com/questions/48493500/can-flexbox-handle-varying-sizes-of-columns-but-consistent-row-height/48496343#48496343
    // For now, manually tuned margins :(
    let mut col1 = Vec::new();
    let mut col2 = Text::new();
    let mut col3 = Text::new();
    let mut col4 = Text::new();
    let mut col5 = Text::new();
    let mut col6 = Text::new();
    let mut col7 = Text::new();

    for (id, mode, departure, duration, comparison, blocked, pct_blocked) in
        data.into_iter().take(30)
    {
        col1.push(Btn::plaintext(id.0.to_string()).build_def(ctx, None));
        col2.add(Line(mode.ongoing_verb()));
        col3.add(Line(departure.ampm_tostring()));
        col4.add(Line(duration.to_string()));
        col5.add_appended(comparison);
        col6.add(Line(blocked.to_string()));
        col7.add(Line(format!("{}%", pct_blocked)));
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
            // TODO The column names aren't lined up at all
            Widget::row(vec![
                Line("Trip ID").draw(ctx).margin_right(10),
                Line("Type").draw(ctx).margin_right(10),
                if sort == SortBy::Departure {
                    Btn::text_fg("Departure").inactive(ctx)
                } else {
                    Btn::text_fg("Departure").build_def(ctx, None)
                }
                .margin_right(10),
                if sort == SortBy::Duration {
                    Btn::text_fg("Duration").inactive(ctx)
                } else {
                    Btn::text_fg("Duration").build_def(ctx, None)
                }
                .margin_right(10),
                Line("Comparison with baseline").draw(ctx).margin_right(10),
                Line("Time spent waiting").draw(ctx).margin_right(10),
                if sort == SortBy::PercentWaiting {
                    Btn::text_fg("Percent of trip spent waiting").inactive(ctx)
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
                col5.draw(ctx).margin_right(10),
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

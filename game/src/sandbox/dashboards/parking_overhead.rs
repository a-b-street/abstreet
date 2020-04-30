use crate::app::App;
use crate::game::{State, Transition};
use crate::info::Tab;
use crate::sandbox::dashboards::trip_table::make_table;
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;
use abstutil::prettyprint_usize;
use ezgui::{Btn, Composite, EventCtx, GfxCtx, Line, Outcome, Text, TextExt, Widget};
use geom::Duration;
use maplit::btreemap;
use sim::{TripID, TripPhaseType};

const ROWS: usize = 20;

// TODO Mostly dupliclated code with trip_table. Find the right generalization.
// TODO Compare all of these things before/after
// TODO Filter out border trips

pub struct ParkingOverhead {
    composite: Composite,
    sort_by: SortBy,
    descending: bool,
    skip: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum SortBy {
    TotalDuration,
    DrivingDuration,
    ParkingDuration,
    WalkingDuration,
    PercentOverhead,
}

impl ParkingOverhead {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let sort_by = SortBy::PercentOverhead;
        let descending = true;
        let skip = 0;
        Box::new(ParkingOverhead {
            composite: make(ctx, app, sort_by, descending, skip),
            sort_by,
            descending,
            skip,
        })
    }

    fn change(&mut self, value: SortBy) {
        self.skip = 0;
        if self.sort_by == value {
            self.descending = !self.descending;
        } else {
            self.sort_by = value;
            self.descending = true;
        }
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = make(ctx, app, self.sort_by, self.descending, self.skip);
        new.restore(ctx, &self.composite);
        self.composite = new;
    }
}

impl State for ParkingOverhead {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Total duration" => {
                    self.change(SortBy::TotalDuration);
                    self.recalc(ctx, app);
                }
                "Driving duration" => {
                    self.change(SortBy::DrivingDuration);
                    self.recalc(ctx, app);
                }
                "Parking duration" => {
                    self.change(SortBy::ParkingDuration);
                    self.recalc(ctx, app);
                }
                "Walking duration" => {
                    self.change(SortBy::WalkingDuration);
                    self.recalc(ctx, app);
                }
                "Percent overhead" => {
                    self.change(SortBy::PercentOverhead);
                    self.recalc(ctx, app);
                }
                "previous trips" => {
                    self.skip -= ROWS;
                    self.recalc(ctx, app);
                }
                "next trips" => {
                    self.skip += ROWS;
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
                    return DashTab::ParkingOverhead.transition(ctx, app, x);
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
    total_duration: Duration,
    driving_duration: Duration,
    parking_duration: Duration,
    walking_duration: Duration,
    percent_overhead: usize,
}

fn make(ctx: &mut EventCtx, app: &App, sort: SortBy, descending: bool, skip: usize) -> Composite {
    // Gather raw data
    let mut data = Vec::new();
    for (id, phases) in app.primary.sim.get_analytics().get_all_trip_phases() {
        let mut total_duration = Duration::ZERO;
        let mut driving_duration = Duration::ZERO;
        let mut parking_duration = Duration::ZERO;
        let mut walking_duration = Duration::ZERO;
        let mut ok = true;
        for p in phases {
            if let Some(t2) = p.end_time {
                let dt = t2 - p.start_time;
                total_duration += dt;
                match p.phase_type {
                    TripPhaseType::Driving => {
                        driving_duration += dt;
                    }
                    TripPhaseType::Walking => {
                        walking_duration += dt;
                    }
                    TripPhaseType::Parking => {
                        parking_duration += dt;
                    }
                    _ => {}
                }
            } else {
                ok = false;
                break;
            }
        }
        if !ok || driving_duration == Duration::ZERO {
            continue;
        }

        data.push(Entry {
            trip: id,
            total_duration,
            driving_duration,
            parking_duration,
            walking_duration,
            percent_overhead: (100.0 * (1.0 - (driving_duration / total_duration))) as usize,
        });
    }

    // Sort
    match sort {
        SortBy::TotalDuration => data.sort_by_key(|x| x.total_duration),
        SortBy::DrivingDuration => data.sort_by_key(|x| x.driving_duration),
        SortBy::ParkingDuration => data.sort_by_key(|x| x.parking_duration),
        SortBy::WalkingDuration => data.sort_by_key(|x| x.walking_duration),
        SortBy::PercentOverhead => data.sort_by_key(|x| x.percent_overhead),
    }
    if descending {
        data.reverse();
    }
    let total_rows = data.len();

    // Render data
    let mut rows = Vec::new();
    for x in data.into_iter().skip(skip).take(ROWS) {
        rows.push((
            x.trip.0.to_string(),
            vec![
                Text::from(Line(x.trip.0.to_string())).render_ctx(ctx),
                Text::from(Line(x.total_duration.to_string())).render_ctx(ctx),
                Text::from(Line(x.driving_duration.to_string())).render_ctx(ctx),
                Text::from(Line(x.parking_duration.to_string())).render_ctx(ctx),
                Text::from(Line(x.walking_duration.to_string())).render_ctx(ctx),
                Text::from(Line(format!("{}%", x.percent_overhead))).render_ctx(ctx),
            ],
        ));
    }

    let btn = |value, name| {
        if sort == value {
            Btn::text_bg2(format!("{} {}", name, if descending { "↓" } else { "↑" }))
                .build(ctx, name, None)
        } else {
            Btn::text_bg2(name).build_def(ctx, None)
        }
    };
    let headers = vec![
        Line("Trip ID").draw(ctx),
        btn(SortBy::TotalDuration, "Total duration"),
        btn(SortBy::DrivingDuration, "Driving duration"),
        btn(SortBy::ParkingDuration, "Parking duration"),
        btn(SortBy::WalkingDuration, "Walking duration"),
        btn(SortBy::PercentOverhead, "Percent overhead"),
    ];

    let mut col = vec![DashTab::ParkingOverhead.picker(ctx)];
    col.push(
        Text::from_multiline(vec![
            Line(
                "Trips taken by car also include time to walk between the building and parking \
                 spot, as well as the time to find parking.",
            ),
            Line("Overhead is 1 - driving time / total time"),
            Line("Ideally, overhead is 0% -- the entire trip is just spent driving."),
            Line(""),
            Line("High overhead could mean:"),
            Line("- the car burned more resources and caused more traffic looking for parking"),
            Line("- somebody with impaired movement had to walk far to reach their vehicle"),
            Line("- the person was inconvenienced"),
            Line(""),
            Line("Note: Trips beginning/ending outside the map have an artifically high overhead,"),
            Line("since the time spent driving off-map are not shown here."),
        ])
        .draw(ctx)
        .margin_below(10),
    );
    col.push(
        Widget::row(vec![
            if skip > 0 {
                Btn::text_fg("<").build(ctx, "previous trips", None)
            } else {
                Btn::text_fg("<").inactive(ctx)
            }
            .margin_right(10),
            format!(
                "{}-{} of {}",
                if total_rows > 0 {
                    prettyprint_usize(skip + 1)
                } else {
                    "0".to_string()
                },
                prettyprint_usize((skip + 1 + ROWS).min(total_rows)),
                prettyprint_usize(total_rows)
            )
            .draw_text(ctx)
            .margin_right(10),
            if skip + 1 + ROWS < total_rows {
                Btn::text_fg(">").build(ctx, "next trips", None)
            } else {
                Btn::text_fg(">").inactive(ctx)
            },
        ])
        .margin_below(5),
    );

    col.extend(make_table(
        ctx,
        app,
        headers,
        rows,
        0.88 * ctx.canvas.window_width,
    ));

    Composite::new(Widget::col(col).bg(app.cs.panel_bg).padding(10))
        .max_size_percent(90, 90)
        .build(ctx)
}

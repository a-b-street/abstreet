use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use crate::info::{OpenTrip, Tab};
use crate::sandbox::dashboards::table::{Col, Filter, Table};
use crate::sandbox::dashboards::trip_table::preview_trip;
use crate::sandbox::dashboards::DashTab;
use crate::sandbox::SandboxMode;
use geom::Duration;
use sim::{TripEndpoint, TripID, TripPhaseType};
use widgetry::{Checkbox, EventCtx, Filler, GfxCtx, Line, Outcome, Panel, Text, Widget};

// TODO Compare all of these things before/after

pub struct ParkingOverhead {
    table: Table<Entry, Filters>,
    panel: Panel,
}

impl ParkingOverhead {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let table = make_table(app);
        let panel = make_panel(ctx, app, &table);
        Box::new(ParkingOverhead { table, panel })
    }

    fn recalc(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut new = make_panel(ctx, app, &self.table);
        new.restore(ctx, &self.panel);
        self.panel = new;
    }
}

impl State for ParkingOverhead {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if self.table.clicked(&x) {
                    self.recalc(ctx, app);
                } else if let Ok(idx) = x.parse::<usize>() {
                    let trip = TripID(idx);
                    let person = app.primary.sim.trip_to_person(trip);
                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::ModifyState(Box::new(move |state, ctx, app| {
                            let sandbox = state.downcast_mut::<SandboxMode>().unwrap();
                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::PersonTrips(person, OpenTrip::single(trip)),
                                &mut actions,
                            );
                        })),
                    ]);
                } else {
                    return DashTab::ParkingOverhead.transition(ctx, app, &x);
                }
            }
            Outcome::Changed => {
                self.table.panel_changed(&self.panel);
                self.recalc(ctx, app);
            }
            _ => {}
        };

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
        preview_trip(g, app, &self.panel);
    }
}

struct Entry {
    trip: TripID,
    total_duration: Duration,
    driving_duration: Duration,
    parking_duration: Duration,
    walking_duration: Duration,
    percent_overhead: usize,
    starts_off_map: bool,
    ends_off_map: bool,
}

struct Filters {
    starts_off_map: bool,
    ends_off_map: bool,
}

fn produce_raw_data(app: &App) -> Vec<Entry> {
    // Gather raw data
    let mut data = Vec::new();
    for (id, phases) in app.primary.sim.get_analytics().get_all_trip_phases() {
        let trip = app.primary.sim.trip_info(id);
        let starts_off_map = match trip.start {
            TripEndpoint::Border(_, _) => true,
            _ => false,
        };
        let ends_off_map = match trip.end {
            TripEndpoint::Border(_, _) => true,
            _ => false,
        };

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
            starts_off_map,
            ends_off_map,
        });
    }
    data
}

fn make_table(app: &App) -> Table<Entry, Filters> {
    let filter: Filter<Entry, Filters> = Filter {
        state: Filters {
            starts_off_map: true,
            ends_off_map: true,
        },
        to_controls: Box::new(move |ctx, _, state| {
            Widget::row(vec![
                Checkbox::switch(ctx, "starting off-map", None, state.starts_off_map),
                Checkbox::switch(ctx, "ending off-map", None, state.ends_off_map),
            ])
        }),
        from_controls: Box::new(|panel| Filters {
            starts_off_map: panel.is_checked("starting off-map"),
            ends_off_map: panel.is_checked("ending off-map"),
        }),
        apply: Box::new(|state, x| {
            if !state.starts_off_map && x.starts_off_map {
                return false;
            }
            if !state.ends_off_map && x.ends_off_map {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        produce_raw_data(app),
        Box::new(|x| x.trip.0.to_string()),
        "Percent overhead",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.trip.0.to_string()));
    table.column(
        "Total duration",
        Box::new(|ctx, _, x| Text::from(Line(x.total_duration.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.total_duration))),
    );
    table.column(
        "Driving duration",
        Box::new(|ctx, _, x| Text::from(Line(x.driving_duration.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.driving_duration))),
    );
    table.column(
        "Parking duration",
        Box::new(|ctx, _, x| Text::from(Line(x.parking_duration.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.parking_duration))),
    );
    table.column(
        "Walking duration",
        Box::new(|ctx, _, x| Text::from(Line(x.walking_duration.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.walking_duration))),
    );
    table.column(
        "Percent overhead",
        Box::new(|ctx, _, x| Text::from(Line(format!("{}%", x.percent_overhead))).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.percent_overhead))),
    );

    table
}

fn make_panel(ctx: &mut EventCtx, app: &App, table: &Table<Entry, Filters>) -> Panel {
    let mut col = vec![DashTab::ParkingOverhead.picker(ctx, app)];
    col.push(
        Widget::row(vec![
            Text::from_multiline(vec![
                Line(
                    "Trips taken by car also include time to walk between the building and \
                     parking spot, as well as the time to find parking.",
                ),
                Line("Overhead is 1 - driving time / total time"),
                Line("Ideally, overhead is 0% -- the entire trip is just spent driving."),
                Line(""),
                Line("High overhead could mean:"),
                Line("- the car burned more resources and caused more traffic looking for parking"),
                Line("- somebody with impaired movement had to walk far to reach their vehicle"),
                Line("- the person was inconvenienced"),
                Line(""),
                Line(
                    "Note: Trips beginning/ending outside the map have an artifically high \
                     overhead,",
                ),
                Line("since the time spent driving off-map isn't shown here."),
            ])
            .draw(ctx),
            Filler::square_width(ctx, 0.15).named("preview"),
        ])
        .evenly_spaced(),
    );
    col.push(table.render(ctx, app));

    Panel::new(Widget::col(col))
        .exact_size_percent(90, 90)
        .build(ctx)
}

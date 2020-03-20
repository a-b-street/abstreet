use crate::app::App;
use crate::colors;
use crate::common::CommonState;
use crate::game::{State, Transition};
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    ManagedWidget, Outcome, Slider, TextExt, VerticalAlignment,
};
use geom::{Circle, Distance, Duration, PolyLine, Time};
use map_model::NORMAL_LANE_THICKNESS;
use popdat::psrc::Mode;
use popdat::{clip_trips, Trip};

// TODO I removed the speed controls from this, for now
pub struct TripsVisualizer {
    composite: Composite,
    trips: Vec<(Trip, PolyLine)>,
    active_trips: Vec<usize>,
}

enum MaybeTrip {
    Success(Trip, PolyLine),
    Failure(String),
}

impl TripsVisualizer {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TripsVisualizer {
        let trips = ctx.loading_screen("load trip data", |_, mut timer| {
            let (all_trips, _) = clip_trips(&app.primary.map, &mut timer);
            let map = &app.primary.map;
            let sim = &app.primary.sim;
            let flags = &app.primary.current_flags.sim_flags;
            let maybe_trips =
                timer.parallelize("calculate paths with geometry", all_trips, |trip| {
                    if let Some(spawn_trip) = trip.to_spawn_trip(map) {
                        let mut rng = flags.make_rng();
                        let spec = spawn_trip.to_trip_spec(&mut rng);
                        let req = sim.trip_spec_to_path_req(&spec, map);
                        if let Some(route) = map
                            .pathfind(req.clone())
                            .and_then(|path| path.trace(map, req.start.dist_along(), None))
                        {
                            MaybeTrip::Success(trip, route)
                        } else {
                            MaybeTrip::Failure(req.to_string())
                        }
                    } else {
                        MaybeTrip::Failure(format!(
                            "{:?} trip from {:?} to {:?}",
                            trip.mode, trip.from, trip.to
                        ))
                    }
                });
            let mut final_trips = Vec::new();
            for maybe in maybe_trips {
                match maybe {
                    MaybeTrip::Success(t, route) => {
                        final_trips.push((t, route));
                    }
                    MaybeTrip::Failure(err) => {
                        timer.warn(format!("Skipping trip: {}", err));
                    }
                }
            }
            final_trips
        });

        TripsVisualizer {
            composite: Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::row(vec![
                        Line("Trips Visualizer").roboto_bold().draw(ctx),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    "Active trips".draw_text(ctx).named("active trips"),
                    ManagedWidget::row(vec![
                        Btn::text_fg("forwards 30 minutes").build_def(ctx, hotkey(Key::RightArrow)),
                        Btn::text_fg("backwards 30 minutes").build_def(ctx, hotkey(Key::LeftArrow)),
                    ])
                    .flex_wrap(ctx, 80),
                    ManagedWidget::slider("time slider"),
                ])
                .padding(10)
                .bg(colors::PANEL_BG),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .slider("time slider", Slider::horizontal(ctx, 150.0, 25.0))
            .build(ctx),
            trips,
            active_trips: Vec::new(),
        }
    }

    fn current_time(&self) -> Time {
        Time::END_OF_DAY.percent_of(self.composite.slider("time slider").get_percent())
    }
}

impl State for TripsVisualizer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        let time = self.current_time();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "forwards 30 minutes" => {
                    self.composite.slider_mut("time slider").set_percent(
                        ctx,
                        (time + Duration::minutes(30))
                            .to_percent(Time::END_OF_DAY)
                            .min(1.0),
                    );
                }
                "backwards 30 minutes" => {
                    self.composite.slider_mut("time slider").set_percent(
                        ctx,
                        (time - Duration::minutes(30))
                            .to_percent(Time::END_OF_DAY)
                            .max(0.0),
                    );
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // TODO Do this more efficiently. ;)
        let new_time = self.current_time();
        if time != new_time {
            self.active_trips = self
                .trips
                .iter()
                .enumerate()
                .filter(|(_, (trip, _))| new_time >= trip.depart_at && new_time <= trip.end_time())
                .map(|(idx, _)| idx)
                .collect();

            self.composite.replace(
                ctx,
                "active trips",
                format!(
                    "{} active trips",
                    prettyprint_usize(self.active_trips.len()),
                )
                .draw_text(ctx)
                .named("active trips"),
            );
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let time = self.current_time();
        let mut batch = GeomBatch::new();
        for idx in &self.active_trips {
            let (trip, pl) = (&self.trips[*idx].0, &self.trips[*idx].1);
            let percent = (time - trip.depart_at) / trip.trip_time;

            let color = match trip.mode {
                Mode::Drive => app.cs.get("unzoomed car"),
                Mode::Walk => app.cs.get("unzoomed pedestrian"),
                Mode::Bike => app.cs.get("unzoomed bike"),
                // Little weird, but close enough.
                Mode::Transit => app.cs.get("unzoomed bus"),
            };
            batch.push(
                color,
                Circle::new(
                    pl.dist_along(percent * pl.length()).0,
                    // Draw bigger circles when zoomed out, but don't go smaller than the lane
                    // once fully zoomed in.
                    (Distance::meters(10.0) / g.canvas.cam_zoom).max(NORMAL_LANE_THICKNESS),
                )
                .to_polygon(),
            );
        }
        batch.draw(g);

        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);
    }
}

use crate::common::{CommonState, SpeedControls};
use crate::game::{State, Transition};
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, layout, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, Line, ModalMenu, Slider, Text,
};
use geom::{Circle, Distance, Duration, PolyLine, Time};
use map_model::LANE_THICKNESS;
use popdat::psrc::Mode;
use popdat::{clip_trips, Trip};

pub struct TripsVisualizer {
    menu: ModalMenu,
    trips: Vec<(Trip, PolyLine)>,
    time_slider: Slider,
    speed: SpeedControls,

    active_trips: Vec<usize>,
}

enum MaybeTrip {
    Success(Trip, PolyLine),
    Failure(String),
}

impl TripsVisualizer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TripsVisualizer {
        let trips = ctx.loading_screen("load trip data", |_, mut timer| {
            let (all_trips, _) = clip_trips(&ui.primary.map, &mut timer);
            let map = &ui.primary.map;
            let sim = &ui.primary.sim;
            let flags = &ui.primary.current_flags.sim_flags;
            let maybe_trips =
                timer.parallelize("calculate paths with geometry", all_trips, |trip| {
                    if let Some(spawn_trip) = trip.to_spawn_trip(map) {
                        let mut rng = flags.make_rng();
                        let (_, spec) = spawn_trip.to_trip_spec(&mut rng);
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
            menu: ModalMenu::new(
                "Trips Visualizer",
                vec![
                    (hotkey(Key::Dot), "forwards 10 seconds"),
                    (hotkey(Key::RightArrow), "forwards 30 minutes"),
                    (hotkey(Key::Comma), "backwards 10 seconds"),
                    (hotkey(Key::LeftArrow), "backwards 30 minutes"),
                    (hotkey(Key::F), "goto start of day"),
                    (hotkey(Key::L), "goto end of day"),
                    (hotkey(Key::Escape), "quit"),
                ],
                ctx,
            )
            .disable_standalone_layout(),
            trips,
            time_slider: Slider::new(150.0, 15.0),
            speed: SpeedControls::new(ctx, ui.primary.current_flags.dev, false),
            active_trips: Vec::new(),
        }
    }

    fn current_time(&self) -> Time {
        Time::END_OF_DAY.percent_of(self.time_slider.get_percent())
    }
}

impl State for TripsVisualizer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let time = self.current_time();

        {
            let mut txt = Text::new();
            txt.add(Line(format!("At {}", time)));
            txt.add(Line(format!(
                "{} active trips",
                prettyprint_usize(self.active_trips.len())
            )));
            self.menu.set_info(ctx, txt);
        }
        self.menu.event(ctx);
        ctx.canvas.handle_event(ctx.input);
        layout::stack_vertically(
            layout::ContainerOrientation::TopRight,
            ctx,
            vec![&mut self.time_slider, &mut self.menu],
        );

        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }

        let ten_secs = Duration::seconds(10.0);
        let thirty_mins = Duration::minutes(30);

        if self.menu.action("quit") {
            return Transition::Pop;
        } else if time != Time::END_OF_DAY && self.menu.action("forwards 10 seconds") {
            self.time_slider
                .set_percent(ctx, (time + ten_secs).to_percent(Time::END_OF_DAY));
        } else if time + thirty_mins <= Time::END_OF_DAY && self.menu.action("forwards 30 minutes")
        {
            self.time_slider
                .set_percent(ctx, (time + thirty_mins).to_percent(Time::END_OF_DAY));
        } else if time != Time::START_OF_DAY && self.menu.action("backwards 10 seconds") {
            self.time_slider
                .set_percent(ctx, (time - ten_secs).to_percent(Time::START_OF_DAY));
        } else if time - thirty_mins >= Time::START_OF_DAY
            && self.menu.action("backwards 30 minutes")
        {
            self.time_slider
                .set_percent(ctx, (time - thirty_mins).to_percent(Time::END_OF_DAY));
        } else if time != Time::START_OF_DAY && self.menu.action("goto start of day") {
            self.time_slider.set_percent(ctx, 0.0);
        } else if time != Time::END_OF_DAY && self.menu.action("goto end of day") {
            self.time_slider.set_percent(ctx, 1.0);
        } else if self.time_slider.event(ctx) {
            // Value changed, fall-through
        } else if let Some(dt) = self.speed.event(ctx, time) {
            // TODO Speed description is briefly weird when we jump backwards with the other
            // control.
            self.time_slider
                .set_percent(ctx, (time + dt).to_percent(Time::END_OF_DAY).min(1.0));
        } else {
            return Transition::Keep;
        }

        // TODO Do this more efficiently. ;)
        let time = self.current_time();
        self.active_trips = self
            .trips
            .iter()
            .enumerate()
            .filter(|(_, (trip, _))| time >= trip.depart_at && time <= trip.end_time())
            .map(|(idx, _)| idx)
            .collect();

        if self.speed.is_paused() {
            Transition::Keep
        } else {
            Transition::KeepWithMode(EventLoopMode::Animation)
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let time = self.current_time();
        let mut batch = GeomBatch::new();
        for idx in &self.active_trips {
            let (trip, pl) = (&self.trips[*idx].0, &self.trips[*idx].1);
            let percent = (time - trip.depart_at) / trip.trip_time;

            let color = match trip.mode {
                Mode::Drive => ui.cs.get("unzoomed car"),
                Mode::Walk => ui.cs.get("unzoomed pedestrian"),
                Mode::Bike => ui.cs.get("unzoomed bike"),
                // Little weird, but close enough.
                Mode::Transit => ui.cs.get("unzoomed bus"),
            };
            batch.push(
                color,
                Circle::new(
                    pl.dist_along(percent * pl.length()).0,
                    // Draw bigger circles when zoomed out, but don't go smaller than the lane
                    // once fully zoomed in.
                    (Distance::meters(10.0) / g.canvas.cam_zoom).max(LANE_THICKNESS),
                )
                .to_polygon(),
            );
        }
        batch.draw(g);

        self.menu.draw(g);
        self.time_slider.draw(g);
        self.speed.draw(g);
        CommonState::draw_osd(g, ui, &ui.primary.current_selection);
    }
}

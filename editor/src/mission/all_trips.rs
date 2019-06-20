use crate::common::{CommonState, SpeedControls};
use crate::mission::trips::{clip_trips, Trip};
use crate::ui::{ShowEverything, UI};
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, ModalMenu, ScreenPt, Slider, Text,
};
use geom::{Circle, Distance, Duration};
use map_model::{PathRequest, LANE_THICKNESS};
use popdat::psrc::Mode;

pub struct TripsVisualizer {
    menu: ModalMenu,
    trips: Vec<Trip>,
    time_slider: Slider,
    speed: SpeedControls,

    active_trips: Vec<usize>,
}

enum MaybeTrip {
    Success(Trip),
    Failure(PathRequest),
}

impl TripsVisualizer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TripsVisualizer {
        let trips = ctx.loading_screen("load trip data", |_, mut timer| {
            let (all_trips, _) = clip_trips(ui, &mut timer);
            let map = &ui.primary.map;
            let maybe_trips =
                timer.parallelize("calculate paths with geometry", all_trips, |mut trip| {
                    let req = trip.path_req(map);
                    if let Some(route) = map
                        .pathfind(req.clone())
                        .and_then(|path| path.trace(map, req.start.dist_along(), None))
                    {
                        trip.route = Some(route);
                        MaybeTrip::Success(trip)
                    } else {
                        MaybeTrip::Failure(req)
                    }
                });
            let mut final_trips = Vec::new();
            for maybe in maybe_trips {
                match maybe {
                    MaybeTrip::Success(t) => {
                        final_trips.push(t);
                    }
                    MaybeTrip::Failure(req) => {
                        timer.warn(format!("Couldn't satisfy {}", req));
                    }
                }
            }
            final_trips
        });

        TripsVisualizer {
            menu: ModalMenu::new(
                "Trips Visualizer",
                vec![
                    (hotkey(Key::Escape), "quit"),
                    (hotkey(Key::Dot), "forwards 10 seconds"),
                    (hotkey(Key::RightArrow), "forwards 30 minutes"),
                    (hotkey(Key::Comma), "backwards 10 seconds"),
                    (hotkey(Key::LeftArrow), "backwards 30 minutes"),
                    (hotkey(Key::F), "goto start of day"),
                    (hotkey(Key::L), "goto end of day"),
                    (hotkey(Key::LeftBracket), "slow down"),
                    (hotkey(Key::RightBracket), "speed up"),
                    (hotkey(Key::Space), "pause/resume"),
                ],
                ctx,
            ),
            trips,
            time_slider: Slider::new(None),
            // TODO hardcoding placement...
            speed: SpeedControls::new(ctx, Some(ScreenPt::new(500.0, 0.0))),
            active_trips: Vec::new(),
        }
    }

    fn current_time(&self) -> Duration {
        self.time_slider.get_percent() * Duration::parse("23:59:59.9").unwrap()
    }

    // Returns None if the we're done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        let time = self.current_time();

        let mut txt = Text::prompt("Trips Visualizer");
        txt.add_line(format!(
            "{} active trips",
            prettyprint_usize(self.active_trips.len())
        ));
        self.menu.handle_event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        if ctx.redo_mouseover() {
            ui.primary.current_selection = ui.recalculate_current_selection(
                ctx,
                &ui.primary.sim,
                &ShowEverything::new(),
                false,
            );
        }

        let last_time = Duration::parse("23:59:59.9").unwrap();
        let ten_secs = Duration::seconds(10.0);
        let thirty_mins = Duration::minutes(30);

        if self.menu.action("quit") {
            return None;
        } else if time != last_time && self.menu.action("forwards 10 seconds") {
            self.time_slider
                .set_percent(ctx, (time + ten_secs) / last_time);
        } else if time + thirty_mins <= last_time && self.menu.action("forwards 30 minutes") {
            self.time_slider
                .set_percent(ctx, (time + thirty_mins) / last_time);
        } else if time != Duration::ZERO && self.menu.action("backwards 10 seconds") {
            self.time_slider
                .set_percent(ctx, (time - ten_secs) / last_time);
        } else if time - thirty_mins >= Duration::ZERO && self.menu.action("backwards 30 minutes") {
            self.time_slider
                .set_percent(ctx, (time - thirty_mins) / last_time);
        } else if time != Duration::ZERO && self.menu.action("goto start of day") {
            self.time_slider.set_percent(ctx, 0.0);
        } else if time != last_time && self.menu.action("goto end of day") {
            self.time_slider.set_percent(ctx, 1.0);
        } else if self.time_slider.event(ctx) {
            // Value changed, fall-through
        } else if let Some(dt) = self.speed.event(ctx, &mut self.menu, time) {
            // TODO Speed description is briefly weird when we jump backwards with the other
            // control.
            self.time_slider
                .set_percent(ctx, ((time + dt) / last_time).min(1.0));
        } else {
            return Some(EventLoopMode::InputOnly);
        }

        // TODO Do this more efficiently. ;)
        let time = self.current_time();
        self.active_trips = self
            .trips
            .iter()
            .enumerate()
            .filter(|(_, trip)| time >= trip.depart_at && time <= trip.end_time())
            .map(|(idx, _)| idx)
            .collect();

        if self.speed.is_paused() {
            Some(EventLoopMode::InputOnly)
        } else {
            Some(EventLoopMode::Animation)
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let time = self.current_time();
        let mut batch = GeomBatch::new();
        for idx in &self.active_trips {
            let trip = &self.trips[*idx];
            let percent = (time - trip.depart_at) / trip.trip_time;

            let pl = trip.route.as_ref().unwrap();
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
        self.time_slider
            .draw(g, Some(Text::from_line(format!("At {}", time))));
        self.speed.draw(g);
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}

use crate::common::CommonState;
use crate::mission::{clip_trips, Trip};
use crate::ui::{ShowEverything, UI};
use abstutil::{prettyprint_usize, Timer};
use ezgui::{Color, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu, Text};
use geom::{Circle, Distance, Duration};
use popdat::PopDat;

pub struct TripsVisualizer {
    menu: ModalMenu,
    trips: Vec<Trip>,
    time: Duration,

    active_trips: Vec<usize>,
}

impl TripsVisualizer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TripsVisualizer {
        let mut timer = Timer::new("initialize popdat");
        let popdat: PopDat = abstutil::read_binary("../data/shapes/popdat", &mut timer)
            .expect("Couldn't load popdat");

        TripsVisualizer {
            menu: ModalMenu::new(
                "Trips Visualizer",
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::Dot), "forwards 1 minute"),
                    (Some(Key::Comma), "backwards 1 minute"),
                    (Some(Key::F), "goto start of day"),
                    (Some(Key::L), "goto end of day"),
                ],
                ctx,
            ),
            trips: clip_trips(&popdat, ui, &mut timer),
            time: Duration::ZERO,
            active_trips: Vec::new(),
        }
    }

    // Returns true if the we're done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        let mut txt = Text::prompt("Trips Visualizer");
        txt.add_line(format!(
            "{} active trips",
            prettyprint_usize(self.active_trips.len())
        ));
        txt.add_line(format!("At {}", self.time));
        self.menu.handle_event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        ui.primary.current_selection =
            ui.handle_mouseover(ctx, &ui.primary.sim, &ShowEverything::new(), false);

        let last_time = Duration::parse("23:59:00.0").unwrap();

        if self.menu.action("quit") {
            return true;
        } else if self.time != last_time && self.menu.action("forwards 1 minute") {
            self.time += Duration::minutes(1);
        } else if self.time != Duration::ZERO && self.menu.action("backwards 1 minute") {
            self.time -= Duration::minutes(1);
        } else if self.time != Duration::ZERO && self.menu.action("goto start of day") {
            self.time = Duration::ZERO;
        } else if self.time != last_time && self.menu.action("goto end of day") {
            self.time = last_time;
        } else {
            return false;
        }

        // TODO Do this more efficiently. ;)
        self.active_trips = self
            .trips
            .iter()
            .enumerate()
            .filter(|(_, trip)| self.time >= trip.depart_at && self.time <= trip.end_time())
            .map(|(idx, _)| idx)
            .collect();

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut batch = GeomBatch::new();
        for idx in &self.active_trips {
            let trip = &self.trips[*idx];
            let from = ui.primary.map.get_b(trip.from);
            let to = ui.primary.map.get_b(trip.to);

            let percent = ((self.time - trip.depart_at) / trip.trip_time) as f32;
            batch.push(
                Color::RED.alpha(1.0 - percent),
                Circle::new(from.polygon.center(), Distance::meters(100.0)).to_polygon(),
            );
            batch.push(
                Color::BLUE.alpha(percent),
                Circle::new(to.polygon.center(), Distance::meters(100.0)).to_polygon(),
            );
        }
        batch.draw(g);

        self.menu.draw(g);
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}

use crate::common::CommonState;
use crate::mission::{clip_trips, Trip};
use crate::ui::{ShowEverything, UI};
use abstutil::{prettyprint_usize, Timer};
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu, Slider, Text};
use geom::{Circle, Distance, Speed};
use popdat::PopDat;

pub struct TripsVisualizer {
    menu: ModalMenu,
    trips: Vec<Trip>,
    slider: Slider,
}

impl TripsVisualizer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TripsVisualizer {
        let mut timer = Timer::new("initialize popdat");
        let popdat: PopDat = abstutil::read_binary("../data/shapes/popdat", &mut timer)
            .expect("Couldn't load popdat");
        // TODO We'll break if there are no matching trips
        let trips = clip_trips(&popdat, ui, 10_000, &mut timer);
        TripsVisualizer {
            menu: ModalMenu::new(
                "Trips Visualizer",
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::Dot), "next trip"),
                    (Some(Key::Comma), "prev trip"),
                    (Some(Key::F), "first trip"),
                    (Some(Key::L), "last trip"),
                ],
                ctx,
            ),
            slider: Slider::new(),
            trips,
        }
    }

    // Returns true if the we're done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        let current = self.slider.get_value(self.trips.len());

        let mut txt = Text::prompt("Trips Visualizer");
        txt.add_line(format!(
            "Trip {}/{}",
            prettyprint_usize(current + 1),
            prettyprint_usize(self.trips.len())
        ));
        let trip = &self.trips[current];
        txt.add_line(format!("Leave at {}", trip.depart_at));
        txt.add_line(format!(
            "Purpose: {:?} -> {:?}",
            trip.purpose.0, trip.purpose.1
        ));
        txt.add_line(format!("Mode: {:?}", trip.mode));
        txt.add_line(format!("Trip time: {}", trip.trip_time));
        txt.add_line(format!("Trip distance: {}", trip.trip_dist));
        txt.add_line(format!(
            "Average speed {}",
            Speed::from_dist_time(trip.trip_dist, trip.trip_time)
        ));
        self.menu.handle_event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        ui.primary.current_selection =
            ui.handle_mouseover(ctx, &ui.primary.sim, &ShowEverything::new(), false);

        if self.menu.action("quit") {
            return true;
        } else if current != self.trips.len() - 1 && self.menu.action("next trip") {
            self.slider.set_value(ctx, current + 1, self.trips.len());
        } else if current != self.trips.len() - 1 && self.menu.action("last trip") {
            self.slider.set_percent(ctx, 1.0);
        } else if current != 0 && self.menu.action("prev trip") {
            self.slider.set_value(ctx, current - 1, self.trips.len());
        } else if current != 0 && self.menu.action("first trip") {
            self.slider.set_percent(ctx, 0.0);
        }

        self.slider.event(ctx);

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let trip = &self.trips[self.slider.get_value(self.trips.len())];
        let from = ui.primary.map.get_b(trip.from);
        let to = ui.primary.map.get_b(trip.to);

        g.draw_polygon(Color::RED, &from.polygon);
        g.draw_polygon(Color::BLUE, &to.polygon);

        // Hard to see the buildings highlighted, so also a big circle...
        g.draw_circle(
            Color::RED.alpha(0.5),
            &Circle::new(from.polygon.center(), Distance::meters(100.0)),
        );
        g.draw_circle(
            Color::BLUE.alpha(0.5),
            &Circle::new(to.polygon.center(), Distance::meters(100.0)),
        );

        self.menu.draw(g);
        self.slider.draw(g);
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}

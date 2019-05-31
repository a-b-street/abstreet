use crate::common::CommonState;
use crate::mission::{clip_trips, Trip};
use crate::ui::{ShowEverything, UI};
use abstutil::{prettyprint_usize, Timer};
use ezgui::{hotkey, Color, EventCtx, GfxCtx, ItemSlider, Key, Text};
use geom::{Circle, Distance, Speed};
use popdat::PopDat;

pub struct TripsVisualizer {
    slider: ItemSlider<Trip>,
}

impl TripsVisualizer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> TripsVisualizer {
        let mut timer = Timer::new("initialize popdat");
        let popdat: PopDat = abstutil::read_binary("../data/shapes/popdat", &mut timer)
            .expect("Couldn't load popdat");
        // TODO We'll break if there are no matching trips
        let trips = clip_trips(&popdat, ui, 10_000, &mut timer);
        TripsVisualizer {
            slider: ItemSlider::new(
                trips,
                "Trips Visualizer",
                "trip",
                vec![(hotkey(Key::Escape), "quit")],
                ctx,
            ),
        }
    }

    // Returns true if the we're done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        let (idx, trip) = self.slider.get();
        let mut txt = Text::prompt("Trips Visualizer");
        txt.add_line(format!(
            "Trip {}/{}",
            prettyprint_usize(idx + 1),
            prettyprint_usize(self.slider.len())
        ));
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

        self.slider.event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        ui.primary.current_selection =
            ui.handle_mouseover(ctx, &ui.primary.sim, &ShowEverything::new(), false);

        if self.slider.action("quit") {
            return true;
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let (_, trip) = self.slider.get();
        let from = trip.from.polygon(&ui.primary.map);
        let to = trip.to.polygon(&ui.primary.map);

        g.draw_polygon(Color::RED, from);
        g.draw_polygon(Color::BLUE, to);

        // Hard to see the buildings/intersections highlighted, so also a big circle...
        g.draw_circle(
            Color::RED.alpha(0.5),
            &Circle::new(from.center(), Distance::meters(100.0)),
        );
        g.draw_circle(
            Color::BLUE.alpha(0.5),
            &Circle::new(to.center(), Distance::meters(100.0)),
        );

        self.slider.draw(g);
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}

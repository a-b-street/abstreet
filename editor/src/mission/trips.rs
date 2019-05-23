use crate::ui::UI;
use abstutil::Timer;
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu, Text};
use geom::{Circle, Distance, Duration, Pt2D};
use popdat::PopDat;

pub struct TripsVisualizer {
    menu: ModalMenu,
    trips: Vec<Trip>,
    current: usize,
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
                    (Some(Key::Dot), "next trip"),
                    (Some(Key::Comma), "prev trip"),
                    (Some(Key::F), "first trip"),
                    (Some(Key::L), "last trip"),
                ],
                ctx,
            ),
            trips: clip_trips(&popdat, ui, &mut timer),
            // TODO We'll break if there are no matching trips
            current: 0,
        }
    }

    // Returns true if the we're done
    pub fn event(&mut self, ctx: &mut EventCtx, _ui: &UI) -> bool {
        let mut txt = Text::prompt("Trips Visualizer");
        txt.add_line(format!(
            "Trip {} starts at {}",
            self.current, self.trips[self.current].depart_at,
        ));
        self.menu.handle_event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return true;
        } else if self.current != self.trips.len() - 1 && self.menu.action("next trip") {
            self.current += 1;
        } else if self.current != self.trips.len() - 1 && self.menu.action("last trip") {
            self.current = self.trips.len() - 1;
        } else if self.current != 0 && self.menu.action("prev trip") {
            self.current -= 1;
        } else if self.current != 0 && self.menu.action("first trip") {
            self.current = 0;
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, _ui: &UI) {
        let trip = &self.trips[self.current];
        g.draw_circle(Color::RED, &Circle::new(trip.from, Distance::meters(100.0)));
        g.draw_circle(Color::BLUE, &Circle::new(trip.to, Distance::meters(100.0)));

        self.menu.draw(g);
    }
}

struct Trip {
    from: Pt2D,
    to: Pt2D,
    depart_at: Duration,
}

fn clip_trips(popdat: &PopDat, ui: &UI, _timer: &mut Timer) -> Vec<Trip> {
    let mut results = Vec::new();
    let bounds = ui.primary.map.get_gps_bounds();
    for trip in &popdat.trips {
        if !bounds.contains(trip.from) || !bounds.contains(trip.to) {
            continue;
        }
        results.push(Trip {
            from: Pt2D::from_gps(trip.from, bounds).unwrap(),
            to: Pt2D::from_gps(trip.to, bounds).unwrap(),
            depart_at: trip.depart_at,
        });
    }
    println!(
        "Clipped {} trips from {}",
        results.len(),
        popdat.trips.len()
    );
    results
}

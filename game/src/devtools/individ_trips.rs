use crate::app::App;
use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::ID;
use ezgui::{hotkey, Color, EventCtx, GfxCtx, ItemSlider, Key, Line, Text};
use geom::{Circle, Distance, Duration, Line, Speed};
use map_model::BuildingID;
use popdat::{clip_trips, psrc, Trip, TripEndpt};
use std::collections::HashMap;

pub struct TripsVisualizer {
    slider: ItemSlider<Trip>,
    bldgs: HashMap<BuildingID, psrc::Parcel>,
}

impl TripsVisualizer {
    pub fn new(ctx: &mut EventCtx, app: &App) -> TripsVisualizer {
        let (trips, bldgs) = ctx.loading_screen("load trip data", |_, mut timer| {
            // TODO We'll break if there are no matching trips
            let (trips, bldgs) = clip_trips(&app.primary.map, &mut timer);
            (
                trips
                    .into_iter()
                    .map(|trip| {
                        let mut txt = Text::new();
                        txt.add(Line(format!("Leave at {}", trip.depart_at)));
                        txt.add(Line(format!(
                            "Purpose: {:?} -> {:?}",
                            trip.purpose.0, trip.purpose.1
                        )));
                        txt.add(Line(format!(
                            "Person {:?}, trip seq {:?}",
                            trip.person, trip.seq
                        )));
                        txt.add(Line(format!("Mode: {:?}", trip.mode)));
                        txt.add(Line(format!("Trip time: {}", trip.trip_time)));
                        txt.add(Line(format!("Trip distance: {}", trip.trip_dist)));
                        if trip.trip_time > Duration::ZERO {
                            txt.add(Line(format!(
                                "Average speed {}",
                                Speed::from_dist_time(trip.trip_dist, trip.trip_time)
                            )));
                        }
                        (trip, txt)
                    })
                    .collect(),
                bldgs,
            )
        });
        TripsVisualizer {
            slider: ItemSlider::new(
                trips,
                "Trips Visualizer",
                "trip",
                vec![(hotkey(Key::Escape), "quit")],
                ctx,
            ),
            bldgs,
        }
    }
}

impl State for TripsVisualizer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.slider.event(ctx);
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        if self.slider.action("quit") {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let (_, trip) = self.slider.get();
        let from = trip.from.polygon(&app.primary.map);
        let to = trip.to.polygon(&app.primary.map);

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

        // For borders, draw the original out-of-bounds points.
        match trip.from {
            TripEndpt::Border(_, pt) => g.draw_line(
                Color::RED,
                Distance::meters(25.0),
                &Line::new(pt, from.center()),
            ),
            TripEndpt::Building(_) => {}
        }
        match trip.to {
            TripEndpt::Border(_, pt) => g.draw_line(
                Color::BLUE,
                Distance::meters(25.0),
                &Line::new(pt, to.center()),
            ),
            TripEndpt::Building(_) => {}
        }

        self.slider.draw(g);
        if let Some(ID::Building(b)) = app.primary.current_selection {
            let mut osd = CommonState::default_osd(ID::Building(b), app);
            if let Some(md) = self.bldgs.get(&b) {
                osd.append(Line(format!(
                    ". {} households, {} employees, {} offstreet parking spaces",
                    md.num_households, md.num_employees, md.offstreet_parking_spaces
                )));
            }
            CommonState::draw_custom_osd(g, app, osd);
        } else {
            CommonState::draw_osd(g, app, &app.primary.current_selection);
        }
    }
}

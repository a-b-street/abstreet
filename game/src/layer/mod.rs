pub mod bus;
mod elevation;
pub mod map;
mod pandemic;
mod parking;
mod population;
pub mod traffic;

use crate::app::App;
use crate::common::{Colorer, HeatmapOptions, Warping};
use crate::game::Transition;
use crate::helpers::ID;
use crate::managed::{ManagedGUIState, WrappedComposite};
use crate::render::MIN_ZOOM_FOR_DETAIL;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GfxCtx, Key, Line, Outcome, Widget,
};
use geom::Time;
use map_model::{BusRouteID, IntersectionID};

// TODO Good ideas in
// https://towardsdatascience.com/top-10-map-types-in-data-visualization-b3a80898ea70

pub enum Layers {
    Inactive,
    ParkingOccupancy(Time, Colorer),
    WorstDelay(Time, Colorer),
    TrafficJams(Time, Colorer),
    CumulativeThroughput(Time, Colorer),
    BikeNetwork(Colorer, Option<Colorer>),
    BusNetwork(Colorer),
    Elevation(Colorer, Drawable),
    Edits(Colorer),
    PopulationMap(Time, population::Options, Drawable, Composite),
    Pandemic(Time, pandemic::Options, Drawable, Composite),

    // These aren't selectable from the main picker; they're particular to some object.
    // TODO They should become something else, like an info panel tab.
    IntersectionDemand(Time, IntersectionID, Drawable, Composite),
    BusRoute(Time, BusRouteID, bus::ShowBusRoute),
}

impl Layers {
    pub fn is_empty(&self) -> bool {
        match self {
            Layers::Inactive => true,
            _ => false,
        }
    }

    // Since Layers is embedded in UI, we have to do this slight trick
    pub fn update(ctx: &mut EventCtx, app: &mut App, minimap: &Composite) -> Option<Transition> {
        let now = app.primary.sim.time();
        match app.layer {
            Layers::ParkingOccupancy(t, _) => {
                if now != t {
                    app.layer = parking::new(ctx, app);
                }
            }
            Layers::WorstDelay(t, _) => {
                if now != t {
                    app.layer = traffic::delay(ctx, app);
                }
            }
            Layers::TrafficJams(t, _) => {
                if now != t {
                    app.layer = traffic::traffic_jams(ctx, app);
                }
            }
            Layers::CumulativeThroughput(t, _) => {
                if now != t {
                    app.layer = traffic::throughput(ctx, app);
                }
            }
            Layers::IntersectionDemand(t, i, _, _) => {
                if now != t {
                    app.layer = traffic::intersection_demand(ctx, app, i);
                }
            }
            Layers::BusRoute(t, id, _) => {
                if now != t {
                    app.layer = bus::ShowBusRoute::new(ctx, app, id);
                }
            }
            Layers::PopulationMap(t, ref opts, _, _) => {
                if now != t {
                    app.layer = population::new(ctx, app, opts.clone());
                }
            }
            Layers::Pandemic(t, ref opts, _, _) => {
                if now != t {
                    app.layer = pandemic::new(ctx, app, opts.clone());
                }
            }
            // No updates needed
            Layers::Inactive
            | Layers::BikeNetwork(_, _)
            | Layers::BusNetwork(_)
            | Layers::Elevation(_, _)
            | Layers::Edits(_) => {}
        };

        match app.layer {
            Layers::ParkingOccupancy(_, ref mut c)
            | Layers::BusNetwork(ref mut c)
            | Layers::Elevation(ref mut c, _)
            | Layers::WorstDelay(_, ref mut c)
            | Layers::TrafficJams(_, ref mut c)
            | Layers::CumulativeThroughput(_, ref mut c)
            | Layers::Edits(ref mut c) => {
                c.legend.align_above(ctx, minimap);
                if c.event(ctx) {
                    app.layer = Layers::Inactive;
                }
            }
            Layers::BikeNetwork(ref mut c1, ref mut maybe_c2) => {
                if let Some(ref mut c2) = maybe_c2 {
                    c2.legend.align_above(ctx, minimap);
                    c1.legend.align_above(ctx, &c2.legend);
                    if c1.event(ctx) || c2.event(ctx) {
                        app.layer = Layers::Inactive;
                    }
                } else {
                    c1.legend.align_above(ctx, minimap);
                    if c1.event(ctx) {
                        app.layer = Layers::Inactive;
                    }
                }
            }
            Layers::BusRoute(_, _, ref mut c) => {
                c.colorer.legend.align_above(ctx, minimap);
                if c.colorer.event(ctx) {
                    app.layer = Layers::Inactive;
                }
            }
            Layers::IntersectionDemand(_, i, _, ref mut c) => {
                c.align_above(ctx, minimap);
                match c.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "intersection demand" => {
                            let id = ID::Intersection(i);
                            return Some(Transition::Push(Warping::new(
                                ctx,
                                id.canonical_point(&app.primary).unwrap(),
                                Some(10.0),
                                Some(id.clone()),
                                &mut app.primary,
                            )));
                        }
                        "X" => {
                            app.layer = Layers::Inactive;
                        }
                        _ => unreachable!(),
                    },
                    None => {}
                }
            }
            Layers::PopulationMap(_, ref mut opts, _, ref mut c) => {
                c.align_above(ctx, minimap);
                match c.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "close" => {
                            app.layer = Layers::Inactive;
                        }
                        _ => unreachable!(),
                    },
                    None => {
                        let new_opts = population::options(c);
                        if *opts != new_opts {
                            app.layer = population::new(ctx, app, new_opts);
                            // Immediately fix the alignment. TODO Do this for all of them, in a
                            // more uniform way
                            if let Layers::PopulationMap(_, _, _, ref mut c) = app.layer {
                                c.align_above(ctx, minimap);
                            }
                        }
                    }
                }
            }
            Layers::Pandemic(_, ref mut opts, _, ref mut c) => {
                c.align_above(ctx, minimap);
                match c.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "close" => {
                            app.layer = Layers::Inactive;
                        }
                        _ => unreachable!(),
                    },
                    None => {
                        let new_opts = pandemic::options(c);
                        if *opts != new_opts {
                            app.layer = pandemic::new(ctx, app, new_opts);
                            // Immediately fix the alignment. TODO Do this for all of them, in a
                            // more uniform way
                            if let Layers::Pandemic(_, _, _, ref mut c) = app.layer {
                                c.align_above(ctx, minimap);
                            }
                        }
                    }
                }
            }
            Layers::Inactive => {}
        }

        None
    }

    // Draw both controls and, if zoomed, the layer contents
    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            Layers::Inactive => {}
            Layers::ParkingOccupancy(_, ref c)
            | Layers::BusNetwork(ref c)
            | Layers::WorstDelay(_, ref c)
            | Layers::TrafficJams(_, ref c)
            | Layers::CumulativeThroughput(_, ref c)
            | Layers::Edits(ref c) => {
                c.draw(g);
            }
            Layers::BikeNetwork(ref c1, ref maybe_c2) => {
                c1.draw(g);
                if let Some(ref c2) = maybe_c2 {
                    c2.draw(g);
                }
            }
            Layers::Elevation(ref c, ref draw) => {
                c.draw(g);
                if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                    g.redraw(draw);
                }
            }
            Layers::PopulationMap(_, _, ref draw, ref composite) => {
                composite.draw(g);
                if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                    g.redraw(draw);
                }
            }
            Layers::Pandemic(_, _, ref draw, ref composite) => {
                composite.draw(g);
                if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                    g.redraw(draw);
                }
            }
            // All of these shouldn't care about zoom
            Layers::IntersectionDemand(_, _, ref draw, ref legend) => {
                g.redraw(draw);
                legend.draw(g);
            }
            Layers::BusRoute(_, _, ref s) => {
                s.draw(g);
            }
        }
    }

    // Just draw contents and do it always
    pub fn draw_minimap(&self, g: &mut GfxCtx) {
        match self {
            Layers::Inactive => {}
            Layers::ParkingOccupancy(_, ref c)
            | Layers::BusNetwork(ref c)
            | Layers::WorstDelay(_, ref c)
            | Layers::TrafficJams(_, ref c)
            | Layers::CumulativeThroughput(_, ref c)
            | Layers::Edits(ref c) => {
                g.redraw(&c.unzoomed);
            }
            Layers::BikeNetwork(ref c1, ref maybe_c2) => {
                g.redraw(&c1.unzoomed);
                if let Some(ref c2) = maybe_c2 {
                    g.redraw(&c2.unzoomed);
                }
            }
            Layers::Elevation(ref c, ref draw) => {
                g.redraw(&c.unzoomed);
                g.redraw(draw);
            }
            Layers::PopulationMap(_, _, ref draw, _) => {
                g.redraw(draw);
            }
            Layers::Pandemic(_, _, ref draw, _) => {
                g.redraw(draw);
            }
            Layers::IntersectionDemand(_, _, _, _) => {}
            Layers::BusRoute(_, _, ref s) => {
                s.draw(g);
            }
        }
    }

    pub fn change_layers(ctx: &mut EventCtx, app: &App) -> Option<Transition> {
        let mut col = vec![Widget::row(vec![
            Line("Layers").small_heading().draw(ctx),
            Btn::plaintext("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ])];

        col.extend(vec![
            Btn::text_fg("None").build_def(ctx, hotkey(Key::N)),
            Btn::text_fg("map edits").build_def(ctx, hotkey(Key::E)),
            Btn::text_fg("worst traffic jams").build_def(ctx, hotkey(Key::J)),
            Btn::text_fg("elevation").build_def(ctx, hotkey(Key::S)),
            Btn::text_fg("parking occupancy").build_def(ctx, hotkey(Key::P)),
            Btn::text_fg("delay").build_def(ctx, hotkey(Key::D)),
            Btn::text_fg("throughput").build_def(ctx, hotkey(Key::T)),
            Btn::text_fg("bike network").build_def(ctx, hotkey(Key::B)),
            Btn::text_fg("bus network").build_def(ctx, hotkey(Key::U)),
            Btn::text_fg("population map").build_def(ctx, hotkey(Key::X)),
        ]);
        if app.primary.sim.get_pandemic_model().is_some() {
            col.push(Btn::text_fg("pandemic model").build_def(ctx, hotkey(Key::Y)));
        }
        if let Some(name) = match app.layer {
            Layers::Inactive => Some("None"),
            Layers::ParkingOccupancy(_, _) => Some("parking occupancy"),
            Layers::WorstDelay(_, _) => Some("delay"),
            Layers::TrafficJams(_, _) => Some("worst traffic jams"),
            Layers::CumulativeThroughput(_, _) => Some("throughput"),
            Layers::BikeNetwork(_, _) => Some("bike network"),
            Layers::BusNetwork(_) => Some("bus network"),
            Layers::Elevation(_, _) => Some("elevation"),
            Layers::Edits(_) => Some("map edits"),
            Layers::PopulationMap(_, _, _, _) => Some("population map"),
            Layers::Pandemic(_, _, _, _) => Some("pandemic model"),
            _ => None,
        } {
            for btn in &mut col {
                if btn.is_btn(name) {
                    *btn = Btn::text_bg2(name).inactive(ctx);
                    break;
                }
            }
        }

        let c = WrappedComposite::new(
            Composite::new(
                Widget::col(col.into_iter().map(|w| w.margin_below(15)).collect())
                    .bg(app.cs.panel_bg)
                    .outline(2.0, Color::WHITE)
                    .padding(10),
            )
            .max_size_percent(35, 70)
            .build(ctx),
        )
        .cb("close", Box::new(|_, _| Some(Transition::Pop)))
        .maybe_cb(
            "None",
            Box::new(|_, app| {
                app.layer = Layers::Inactive;
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "parking occupancy",
            Box::new(|ctx, app| {
                app.layer = parking::new(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "delay",
            Box::new(|ctx, app| {
                app.layer = traffic::delay(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "worst traffic jams",
            Box::new(|ctx, app| {
                app.layer = traffic::traffic_jams(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "throughput",
            Box::new(|ctx, app| {
                app.layer = traffic::throughput(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "bike network",
            Box::new(|ctx, app| {
                app.layer = map::bike_network(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "bus network",
            Box::new(|ctx, app| {
                app.layer = map::bus_network(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "elevation",
            Box::new(|ctx, app| {
                app.layer = elevation::new(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "map edits",
            Box::new(|ctx, app| {
                app.layer = map::edits(ctx, app);
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "population map",
            Box::new(|ctx, app| {
                app.layer = population::new(
                    ctx,
                    app,
                    population::Options {
                        heatmap: Some(HeatmapOptions::new()),
                    },
                );
                Some(Transition::Pop)
            }),
        )
        .maybe_cb(
            "pandemic model",
            Box::new(|ctx, app| {
                app.layer = pandemic::new(
                    ctx,
                    app,
                    pandemic::Options {
                        heatmap: Some(HeatmapOptions::new()),
                        state: pandemic::SEIR::Infected,
                    },
                );
                Some(Transition::Pop)
            }),
        );
        Some(Transition::Push(ManagedGUIState::over_map(c)))
    }
}

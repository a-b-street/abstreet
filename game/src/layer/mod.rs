pub mod bus;
mod elevation;
pub mod map;
mod pandemic;
mod parking;
mod population;
pub mod traffic;

use crate::app::App;
use crate::common::{Colorer, HeatmapOptions, Warping};
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::ID;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GfxCtx, Key, Line, Outcome, Widget,
};
use geom::Time;
use map_model::{BusRouteID, IntersectionID};

// TODO Good ideas in
// https://towardsdatascience.com/top-10-map-types-in-data-visualization-b3a80898ea70

pub trait Layer {
    fn name(&self) -> Option<&'static str>;
    fn update(&self, ctx: &mut EventCtx, app: &App) -> Option<Box<dyn Layer>>;
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome>;
    // Draw both controls and, if zoomed, the layer contents
    fn draw(&self, g: &mut GfxCtx, app: &App);
    // Just draw contents and do it always
    fn draw_minimap(&self, g: &mut GfxCtx);
}

pub enum LayerOutcome {
    Close,
    Transition(Transition),
}

pub enum Layers {
    Generic(Box<dyn Layer>),
    ParkingOccupancy {
        time: Time,
        onstreet: bool,
        offstreet: bool,
        unzoomed: Drawable,
        composite: Composite,
    },
    CumulativeThroughput {
        time: Time,
        compare: bool,
        unzoomed: Drawable,
        composite: Composite,
    },
    WorstDelay(Time, Colorer),
    TrafficJams(Time, Colorer),
    Backpressure(Time, Colorer),
    Elevation(Colorer, Drawable),
    PopulationMap(Time, population::Options, Drawable, Composite),
    Pandemic(Time, pandemic::Options, Drawable, Composite),

    // These aren't selectable from the main picker; they're particular to some object.
    // TODO They should become something else, like an info panel tab.
    IntersectionDemand(Time, IntersectionID, Drawable, Composite),
    BusRoute(Time, BusRouteID, bus::ShowBusRoute),
}

impl Layers {
    // Since Layers is embedded in UI, we have to do this slight trick
    pub fn update(ctx: &mut EventCtx, app: &mut App, minimap: &Composite) -> Option<Transition> {
        if app.layer.is_none() {
            return None;
        }
        let now = app.primary.sim.time();
        match app.layer.as_ref().unwrap() {
            Layers::Generic(ref l) => {
                if let Some(new) = l.update(ctx, app) {
                    app.layer = Some(Layers::Generic(new));
                }
            }
            Layers::ParkingOccupancy {
                time,
                onstreet,
                offstreet,
                ..
            } => {
                if now != *time {
                    app.layer = Some(parking::new(ctx, app, *onstreet, *offstreet));
                }
            }
            Layers::WorstDelay(t, _) => {
                if now != *t {
                    app.layer = Some(traffic::delay(ctx, app));
                }
            }
            Layers::TrafficJams(t, _) => {
                if now != *t {
                    app.layer = Some(traffic::traffic_jams(ctx, app));
                }
            }
            Layers::CumulativeThroughput { time, compare, .. } => {
                if now != *time {
                    app.layer = Some(traffic::throughput(ctx, app, *compare));
                }
            }
            Layers::Backpressure(t, _) => {
                if now != *t {
                    app.layer = Some(traffic::backpressure(ctx, app));
                }
            }
            Layers::IntersectionDemand(t, i, _, _) => {
                if now != *t {
                    app.layer = Some(traffic::intersection_demand(ctx, app, *i));
                }
            }
            Layers::BusRoute(t, id, _) => {
                if now != *t {
                    app.layer = Some(bus::ShowBusRoute::new(ctx, app, *id));
                }
            }
            Layers::PopulationMap(t, opts, _, _) => {
                if now != *t {
                    app.layer = Some(population::new(ctx, app, opts.clone()));
                }
            }
            Layers::Pandemic(t, opts, _, _) => {
                if now != *t {
                    app.layer = Some(pandemic::new(ctx, app, opts.clone()));
                }
            }
            // No updates needed
            Layers::Elevation(_, _) => {}
        };

        // TODO Since Layers is embedded in UI, we have to do this slight trick
        let mut layer = app.layer.take().unwrap();
        if let Layers::Generic(ref mut l) = layer {
            match l.event(ctx, app, minimap) {
                Some(LayerOutcome::Close) => {
                    app.layer = None;
                    return None;
                }
                Some(LayerOutcome::Transition(t)) => {
                    app.layer = Some(layer);
                    return Some(t);
                }
                None => {}
            }
        }
        app.layer = Some(layer);

        match app.layer.as_mut().unwrap() {
            Layers::Generic(_) => {}
            Layers::Elevation(ref mut c, _)
            | Layers::WorstDelay(_, ref mut c)
            | Layers::TrafficJams(_, ref mut c)
            | Layers::Backpressure(_, ref mut c) => {
                c.legend.align_above(ctx, minimap);
                if c.event(ctx) {
                    app.layer = None;
                }
            }
            Layers::ParkingOccupancy {
                ref mut composite,
                onstreet,
                offstreet,
                ..
            } => {
                composite.align_above(ctx, minimap);
                match composite.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "close" => {
                            app.layer = None;
                        }
                        _ => unreachable!(),
                    },
                    None => {
                        let new_onstreet = composite.is_checked("On-street spots");
                        let new_offstreet = composite.is_checked("Off-street spots");
                        if *onstreet != new_onstreet || *offstreet != new_offstreet {
                            app.layer = Some(parking::new(ctx, app, new_onstreet, new_offstreet));
                            // Immediately fix the alignment. TODO Do this for all of them, in a
                            // more uniform way
                            if let Some(Layers::ParkingOccupancy {
                                ref mut composite, ..
                            }) = app.layer
                            {
                                composite.align_above(ctx, minimap);
                            }
                        }
                    }
                }
            }
            Layers::CumulativeThroughput {
                ref mut composite,
                compare,
                ..
            } => {
                composite.align_above(ctx, minimap);
                match composite.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "close" => {
                            app.layer = None;
                        }
                        _ => unreachable!(),
                    },
                    None => {
                        let new_compare = composite.has_widget("Compare before edits")
                            && composite.is_checked("Compare before edits");
                        if new_compare != *compare {
                            app.layer = Some(traffic::throughput(ctx, app, new_compare));
                            // Immediately fix the alignment. TODO Do this for all of them, in a
                            // more uniform way
                            if let Some(Layers::CumulativeThroughput {
                                ref mut composite, ..
                            }) = app.layer
                            {
                                composite.align_above(ctx, minimap);
                            }
                        }
                    }
                }
            }
            Layers::BusRoute(_, _, ref mut c) => {
                c.colorer.legend.align_above(ctx, minimap);
                if c.colorer.event(ctx) {
                    app.layer = None;
                }
            }
            Layers::IntersectionDemand(_, i, _, ref mut c) => {
                c.align_above(ctx, minimap);
                match c.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "intersection demand" => {
                            let id = ID::Intersection(*i);
                            return Some(Transition::Push(Warping::new(
                                ctx,
                                id.canonical_point(&app.primary).unwrap(),
                                Some(10.0),
                                Some(id.clone()),
                                &mut app.primary,
                            )));
                        }
                        "X" => {
                            app.layer = None;
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
                            app.layer = None;
                        }
                        _ => unreachable!(),
                    },
                    None => {
                        let new_opts = population::options(c);
                        if *opts != new_opts {
                            app.layer = Some(population::new(ctx, app, new_opts));
                            // Immediately fix the alignment. TODO Do this for all of them, in a
                            // more uniform way
                            if let Some(Layers::PopulationMap(_, _, _, ref mut c)) = app.layer {
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
                            app.layer = None;
                        }
                        _ => unreachable!(),
                    },
                    None => {
                        let new_opts = pandemic::options(c);
                        if *opts != new_opts {
                            app.layer = Some(pandemic::new(ctx, app, new_opts));
                            // Immediately fix the alignment. TODO Do this for all of them, in a
                            // more uniform way
                            if let Some(Layers::Pandemic(_, _, _, ref mut c)) = app.layer {
                                c.align_above(ctx, minimap);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    // Draw both controls and, if zoomed, the layer contents
    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        match self {
            Layers::Generic(ref l) => {
                l.draw(g, app);
            }
            Layers::WorstDelay(_, ref c)
            | Layers::TrafficJams(_, ref c)
            | Layers::Backpressure(_, ref c) => {
                c.draw(g, app);
            }
            Layers::Elevation(ref c, ref draw) => {
                c.draw(g, app);
                if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                    g.redraw(draw);
                }
            }
            Layers::ParkingOccupancy {
                ref unzoomed,
                ref composite,
                ..
            } => {
                composite.draw(g);
                if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                    g.redraw(unzoomed);
                }
            }
            Layers::CumulativeThroughput {
                ref unzoomed,
                ref composite,
                ..
            } => {
                composite.draw(g);
                if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                    g.redraw(unzoomed);
                }
            }
            Layers::PopulationMap(_, _, ref draw, ref composite) => {
                composite.draw(g);
                if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                    g.redraw(draw);
                }
            }
            Layers::Pandemic(_, _, ref draw, ref composite) => {
                composite.draw(g);
                if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                    g.redraw(draw);
                }
            }
            // All of these shouldn't care about zoom
            Layers::IntersectionDemand(_, _, ref draw, ref legend) => {
                g.redraw(draw);
                legend.draw(g);
            }
            Layers::BusRoute(_, _, ref s) => {
                s.draw(g, app);
            }
        }
    }

    // Just draw contents and do it always
    pub fn draw_minimap(&self, g: &mut GfxCtx, app: &App) {
        match self {
            Layers::Generic(ref l) => {
                l.draw_minimap(g);
            }
            Layers::WorstDelay(_, ref c)
            | Layers::TrafficJams(_, ref c)
            | Layers::Backpressure(_, ref c) => {
                g.redraw(&c.unzoomed);
            }
            Layers::ParkingOccupancy { ref unzoomed, .. } => {
                g.redraw(unzoomed);
            }
            Layers::CumulativeThroughput { ref unzoomed, .. } => {
                g.redraw(unzoomed);
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
                s.draw(g, app);
            }
        }
    }

    pub fn change_layers(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
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
            Btn::text_fg("backpressure").build_def(ctx, hotkey(Key::Z)),
            Btn::text_fg("bike network").build_def(ctx, hotkey(Key::B)),
            Btn::text_fg("bus network").build_def(ctx, hotkey(Key::U)),
            Btn::text_fg("population map").build_def(ctx, hotkey(Key::X)),
            Btn::text_fg("amenities").build_def(ctx, hotkey(Key::A)),
        ]);
        if app.primary.sim.get_pandemic_model().is_some() {
            col.push(Btn::text_fg("pandemic model").build_def(ctx, hotkey(Key::Y)));
        }
        if let Some(name) = match app.layer {
            None => Some("None"),
            Some(Layers::Generic(ref l)) => l.name(),
            Some(Layers::ParkingOccupancy { .. }) => Some("parking occupancy"),
            Some(Layers::WorstDelay(_, _)) => Some("delay"),
            Some(Layers::TrafficJams(_, _)) => Some("worst traffic jams"),
            Some(Layers::CumulativeThroughput { .. }) => Some("throughput"),
            Some(Layers::Backpressure(_, _)) => Some("backpressure"),
            Some(Layers::Elevation(_, _)) => Some("elevation"),
            Some(Layers::PopulationMap(_, _, _, _)) => Some("population map"),
            Some(Layers::Pandemic(_, _, _, _)) => Some("pandemic model"),
            Some(Layers::IntersectionDemand(_, _, _, _)) => None,
            Some(Layers::BusRoute(_, _, _)) => None,
        } {
            for btn in &mut col {
                if btn.is_btn(name) {
                    *btn = Btn::text_bg2(name).inactive(ctx);
                    break;
                }
            }
        }

        Box::new(PickLayer {
            composite: Composite::new(
                Widget::col(col.into_iter().map(|w| w.margin_below(15)).collect())
                    .bg(app.cs.panel_bg)
                    .outline(2.0, Color::WHITE)
                    .padding(10),
            )
            .max_size_percent(35, 70)
            .build(ctx),
        })
    }
}

pub struct PickLayer {
    composite: Composite,
}

impl State for PickLayer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {}
                "None" => {
                    app.layer = None;
                }
                "parking occupancy" => {
                    app.layer = Some(parking::new(ctx, app, true, true));
                }
                "delay" => {
                    app.layer = Some(traffic::delay(ctx, app));
                }
                "worst traffic jams" => {
                    app.layer = Some(traffic::traffic_jams(ctx, app));
                }
                "throughput" => {
                    app.layer = Some(traffic::throughput(ctx, app, false));
                }
                "backpressure" => {
                    app.layer = Some(traffic::backpressure(ctx, app));
                }
                "bike network" => {
                    app.layer = Some(Layers::Generic(map::BikeNetwork::new(ctx, app)));
                }
                "bus network" => {
                    app.layer = Some(Layers::Generic(map::Static::bus_network(ctx, app)));
                }
                "elevation" => {
                    app.layer = Some(elevation::new(ctx, app));
                }
                "map edits" => {
                    app.layer = Some(Layers::Generic(map::Static::edits(ctx, app)));
                }
                "amenities" => {
                    app.layer = Some(Layers::Generic(map::Static::amenities(ctx, app)));
                }
                "population map" => {
                    app.layer = Some(population::new(
                        ctx,
                        app,
                        population::Options {
                            heatmap: Some(HeatmapOptions::new()),
                        },
                    ));
                }
                "pandemic model" => {
                    app.layer = Some(pandemic::new(
                        ctx,
                        app,
                        pandemic::Options {
                            heatmap: Some(HeatmapOptions::new()),
                            state: pandemic::SEIR::Infected,
                        },
                    ));
                }
                _ => unreachable!(),
            },
            None => {
                if self.composite.clicked_outside(ctx) {
                    return Transition::Pop;
                }
                return Transition::Keep;
            }
        }
        Transition::Pop
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

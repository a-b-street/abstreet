pub mod bus;
mod elevation;
pub mod map;
mod pandemic;
mod parking;
mod population;
pub mod traffic;

use crate::app::App;
use crate::common::HeatmapOptions;
use crate::game::{DrawBaselayer, State, Transition};
use ezgui::{hotkey, Btn, Color, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Widget};

// TODO Good ideas in
// https://towardsdatascience.com/top-10-map-types-in-data-visualization-b3a80898ea70

pub trait Layer {
    fn name(&self) -> Option<&'static str>;
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
}

impl Layers {
    // Since Layers is embedded in UI, we have to do this slight trick
    pub fn update(ctx: &mut EventCtx, app: &mut App, minimap: &Composite) -> Option<Transition> {
        if app.layer.is_none() {
            return None;
        }
        let now = app.primary.sim.time();
        match app.layer.as_ref().unwrap() {
            Layers::Generic(_) => {}
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
        }

        None
    }

    // Draw both controls and, if zoomed, the layer contents
    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        match self {
            Layers::Generic(ref l) => {
                l.draw(g, app);
            }
        }
    }

    // Just draw contents and do it always
    pub fn draw_minimap(&self, g: &mut GfxCtx, app: &App) {
        match self {
            Layers::Generic(ref l) => {
                l.draw_minimap(g);
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
                    app.layer = Some(Layers::Generic(Box::new(parking::Occupancy::new(
                        ctx, app, true, true,
                    ))));
                }
                "delay" => {
                    app.layer = Some(Layers::Generic(Box::new(traffic::Dynamic::delay(ctx, app))));
                }
                "worst traffic jams" => {
                    app.layer = Some(Layers::Generic(Box::new(traffic::Dynamic::traffic_jams(
                        ctx, app,
                    ))));
                }
                "throughput" => {
                    app.layer = Some(Layers::Generic(Box::new(traffic::Throughput::new(
                        ctx, app, false,
                    ))));
                }
                "backpressure" => {
                    app.layer = Some(Layers::Generic(Box::new(traffic::Dynamic::backpressure(
                        ctx, app,
                    ))));
                }
                "bike network" => {
                    app.layer = Some(Layers::Generic(Box::new(map::BikeNetwork::new(ctx, app))));
                }
                "bus network" => {
                    app.layer = Some(Layers::Generic(Box::new(map::Static::bus_network(
                        ctx, app,
                    ))));
                }
                "elevation" => {
                    app.layer = Some(Layers::Generic(Box::new(elevation::Elevation::new(
                        ctx, app,
                    ))));
                }
                "map edits" => {
                    app.layer = Some(Layers::Generic(Box::new(map::Static::edits(ctx, app))));
                }
                "amenities" => {
                    app.layer = Some(Layers::Generic(Box::new(map::Static::amenities(ctx, app))));
                }
                "population map" => {
                    app.layer = Some(Layers::Generic(Box::new(population::PopulationMap::new(
                        ctx,
                        app,
                        population::Options {
                            heatmap: Some(HeatmapOptions::new()),
                        },
                    ))));
                }
                "pandemic model" => {
                    app.layer = Some(Layers::Generic(Box::new(pandemic::Pandemic::new(
                        ctx,
                        app,
                        pandemic::Options {
                            heatmap: Some(HeatmapOptions::new()),
                            state: pandemic::SEIR::Infected,
                        },
                    ))));
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

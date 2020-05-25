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
use crate::helpers::hotkey_btn;
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

// TODO Just return a bool for closed? Less readable...
pub enum LayerOutcome {
    Close,
}

pub struct PickLayer {
    composite: Composite,
}

impl PickLayer {
    pub fn update(ctx: &mut EventCtx, app: &mut App, minimap: &Composite) -> Option<Transition> {
        if app.layer.is_none() {
            return None;
        }

        // TODO Since the Layer is embedded in UI, we have to do this slight trick
        let mut layer = app.layer.take().unwrap();
        match layer.event(ctx, app, minimap) {
            Some(LayerOutcome::Close) => {
                app.layer = None;
                return None;
            }
            None => {}
        }
        app.layer = Some(layer);

        None
    }

    pub fn pick(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let mut col = vec![Widget::row(vec![
            Line("Layers").small_heading().draw(ctx),
            Btn::plaintext("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ])];

        col.extend(vec![
            hotkey_btn(ctx, app, "None", Key::N),
            hotkey_btn(ctx, app, "map edits", Key::E),
            hotkey_btn(ctx, app, "worst traffic jams", Key::J),
            hotkey_btn(ctx, app, "elevation", Key::S),
            hotkey_btn(ctx, app, "parking occupancy", Key::P),
            hotkey_btn(ctx, app, "delay", Key::D),
            hotkey_btn(ctx, app, "throughput", Key::T),
            hotkey_btn(ctx, app, "backpressure", Key::Z),
            hotkey_btn(ctx, app, "bike network", Key::B),
            hotkey_btn(ctx, app, "bus network", Key::U),
            hotkey_btn(ctx, app, "population map", Key::X),
            hotkey_btn(ctx, app, "amenities", Key::A),
        ]);
        if app.primary.sim.get_pandemic_model().is_some() {
            col.push(hotkey_btn(ctx, app, "pandemic model", Key::Y));
        }
        if let Some(name) = match app.layer {
            None => Some("None"),
            Some(ref l) => l.name(),
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

impl State for PickLayer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {}
                "None" => {
                    app.layer = None;
                }
                "parking occupancy" => {
                    app.layer = Some(Box::new(parking::Occupancy::new(ctx, app, true, true)));
                }
                "delay" => {
                    app.layer = Some(Box::new(traffic::Dynamic::delay(ctx, app)));
                }
                "worst traffic jams" => {
                    app.layer = Some(Box::new(traffic::Dynamic::traffic_jams(ctx, app)));
                }
                "throughput" => {
                    app.layer = Some(Box::new(traffic::Throughput::new(ctx, app, false)));
                }
                "backpressure" => {
                    app.layer = Some(Box::new(traffic::Dynamic::backpressure(ctx, app)));
                }
                "bike network" => {
                    app.layer = Some(Box::new(map::BikeNetwork::new(ctx, app)));
                }
                "bus network" => {
                    app.layer = Some(Box::new(map::Static::bus_network(ctx, app)));
                }
                "elevation" => {
                    app.layer = Some(Box::new(elevation::Elevation::new(ctx, app)));
                }
                "map edits" => {
                    app.layer = Some(Box::new(map::Static::edits(ctx, app)));
                }
                "amenities" => {
                    app.layer = Some(Box::new(map::Static::amenities(ctx, app)));
                }
                "population map" => {
                    app.layer = Some(Box::new(population::PopulationMap::new(
                        ctx,
                        app,
                        population::Options {
                            heatmap: Some(HeatmapOptions::new()),
                        },
                    )));
                }
                "pandemic model" => {
                    app.layer = Some(Box::new(pandemic::Pandemic::new(
                        ctx,
                        app,
                        pandemic::Options {
                            heatmap: Some(HeatmapOptions::new()),
                            state: pandemic::SEIR::Infected,
                        },
                    )));
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

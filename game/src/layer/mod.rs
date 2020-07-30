mod elevation;
pub mod map;
mod pandemic;
mod parking;
mod population;
pub mod traffic;
pub mod transit;

use crate::app::App;
use crate::common::HeatmapOptions;
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::hotkey_btn;
use ezgui::{hotkey, Btn, Composite, EventCtx, GfxCtx, Key, Line, Outcome, TextExt, Widget};

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

impl dyn Layer {
    fn simple_event(
        ctx: &mut EventCtx,
        minimap: &Composite,
        composite: &mut Composite,
    ) -> Option<LayerOutcome> {
        composite.align_above(ctx, minimap);
        match composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => Some(LayerOutcome::Close),
                _ => unreachable!(),
            },
            None => None,
        }
    }
}

// TODO Just return a bool for closed? Less readable...
pub enum LayerOutcome {
    Close,
}

// TODO Maybe overkill, but could embed a minimap and preview the layer on hover
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
        let mut col = vec![Widget::custom_row(vec![
            Line("Layers").small_heading().draw(ctx),
            Btn::plaintext("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ])];

        let current = match app.layer {
            None => "None",
            Some(ref l) => l.name().unwrap_or(""),
        };
        let btn = |name: &str, key| {
            if name == current {
                Btn::text_bg2(name).inactive(ctx)
            } else {
                hotkey_btn(ctx, app, name, key)
            }
        };

        col.push(btn("None", Key::N));

        col.push(
            Widget::custom_row(vec![
                Widget::col(vec![
                    "Traffic".draw_text(ctx),
                    btn("delay", Key::D),
                    btn("throughput", Key::T),
                    btn("traffic jams", Key::J),
                ]),
                Widget::col(vec![
                    "Map".draw_text(ctx),
                    btn("map edits", Key::E),
                    btn("parking occupancy", Key::P),
                    btn("bike network", Key::B),
                    btn("transit network", Key::U),
                    btn("population map", Key::X),
                    btn("no sidewalks", Key::S),
                ]),
            ])
            .evenly_spaced(),
        );

        col.extend(vec![
            "Experimental".draw_text(ctx),
            btn("amenities", Key::A),
            btn("backpressure", Key::Z),
            btn("elevation", Key::V),
        ]);
        if app.primary.sim.get_pandemic_model().is_some() {
            col.push(btn("pandemic model", Key::Y));
        }

        Box::new(PickLayer {
            composite: Composite::new(Widget::col(col))
                .exact_size_percent(35, 70)
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
                    app.layer = Some(Box::new(parking::Occupancy::new(
                        ctx, app, true, true, true, false,
                    )));
                }
                "delay" => {
                    app.layer = Some(Box::new(traffic::Delay::new(ctx, app, false)));
                }
                "traffic jams" => {
                    app.layer = Some(Box::new(traffic::TrafficJams::new(ctx, app)));
                }
                "throughput" => {
                    app.layer = Some(Box::new(traffic::Throughput::new(ctx, app, false)));
                }
                "backpressure" => {
                    app.layer = Some(Box::new(traffic::Backpressure::new(ctx, app)));
                }
                "bike network" => {
                    app.layer = Some(Box::new(map::BikeNetwork::new(ctx, app)));
                }
                "transit network" => {
                    app.layer = Some(Box::new(transit::TransitNetwork::new(
                        ctx, app, false, true, true,
                    )));
                }
                "elevation" => {
                    app.layer = Some(Box::new(elevation::Elevation::new(ctx, app)));
                }
                "no sidewalks" => {
                    app.layer = Some(Box::new(map::Static::no_sidewalks(ctx, app)));
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

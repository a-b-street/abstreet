use map_gui::tools::{grey_out_map, HeatmapOptions};
use sim::AgentType;
use widgetry::{
    DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, StyledButtons, TextExt,
    Widget,
};

use crate::app::{App, Transition};
use crate::sandbox::dashboards;

mod elevation;
pub mod favorites;
pub mod map;
mod pandemic;
mod parking;
mod population;
pub mod traffic;
pub mod transit;

// TODO Good ideas in
// https://towardsdatascience.com/top-10-map-types-in-data-visualization-b3a80898ea70

pub trait Layer {
    fn name(&self) -> Option<&'static str>;
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App, minimap: &Panel)
        -> Option<LayerOutcome>;
    // Draw both controls and, if zoomed, the layer contents
    fn draw(&self, g: &mut GfxCtx, app: &App);
    // Just draw contents and do it always
    fn draw_minimap(&self, g: &mut GfxCtx);
}

impl dyn Layer {
    fn simple_event(
        ctx: &mut EventCtx,
        minimap: &Panel,
        panel: &mut Panel,
    ) -> Option<LayerOutcome> {
        panel.align_above(ctx, minimap);
        match panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Some(LayerOutcome::Close),
                _ => unreachable!(),
            },
            _ => None,
        }
    }
}

// TODO Just return a bool for closed? Less readable...
pub enum LayerOutcome {
    Close,
    Replace(Box<dyn Layer>),
}

// TODO Maybe overkill, but could embed a minimap and preview the layer on hover
pub struct PickLayer {
    panel: Panel,
}

impl PickLayer {
    pub fn update(ctx: &mut EventCtx, app: &mut App, minimap: &Panel) -> Option<Transition> {
        if app.primary.layer.is_none() {
            return None;
        }

        // TODO Since the Layer is embedded in App, we have to do this slight trick
        let mut layer = app.primary.layer.take().unwrap();
        match layer.event(ctx, app, minimap) {
            Some(LayerOutcome::Close) => {
                app.primary.layer = None;
                return None;
            }
            Some(LayerOutcome::Replace(l)) => {
                app.primary.layer = Some(l);
                return None;
            }
            None => {}
        }
        app.primary.layer = Some(layer);

        None
    }

    pub fn pick(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut col = vec![Widget::custom_row(vec![
            Line("Layers").small_heading().draw(ctx),
            ctx.style().btn_close_widget(ctx),
        ])];

        let current = match app.primary.layer {
            None => "None",
            Some(ref l) => l.name().unwrap_or(""),
        };
        let btn = |name: &str, key| {
            ctx.style()
                .btn_solid_dark_text(name)
                .hotkey(key)
                .disabled(name == current)
                .build_widget(ctx, name)
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
                    btn("favorite buildings", Key::F),
                ]),
            ])
            .evenly_spaced(),
        );

        col.push(
            Widget::custom_row(vec![
                Widget::col(vec![
                    "Experimental".draw_text(ctx),
                    btn("amenities", Key::A),
                    btn("backpressure", Key::Z),
                    btn("elevation", Key::V),
                    btn("parking efficiency", Key::O),
                    btn("blackholes", Key::L),
                    btn("congestion caps", Key::C),
                    if app.primary.sim.get_pandemic_model().is_some() {
                        btn("pandemic model", Key::Y)
                    } else {
                        Widget::nothing()
                    },
                ]),
                Widget::col(vec![
                    "Data".draw_text(ctx),
                    btn("traffic signal demand", Key::M),
                    btn("commuter patterns", Key::R),
                ]),
            ])
            .evenly_spaced(),
        );

        Box::new(PickLayer {
            panel: Panel::new(Widget::col(col))
                .exact_size_percent(35, 70)
                .build(ctx),
        })
    }
}

impl State<App> for PickLayer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {}
                "None" => {
                    app.primary.layer = None;
                }
                "amenities" => {
                    app.primary.layer = Some(Box::new(map::Static::amenities(ctx, app)));
                }
                "backpressure" => {
                    app.primary.layer = Some(Box::new(traffic::Backpressure::new(ctx, app)));
                }
                "bike network" => {
                    app.primary.layer = Some(Box::new(map::BikeNetwork::new(ctx, app)));
                }
                "delay" => {
                    app.primary.layer = Some(Box::new(traffic::Delay::new(ctx, app)));
                }
                "elevation" => {
                    app.primary.layer = Some(Box::new(elevation::Elevation::new(ctx, app)));
                }
                "map edits" => {
                    app.primary.layer = Some(Box::new(map::Static::edits(ctx, app)));
                }
                "no sidewalks" => {
                    app.primary.layer = Some(Box::new(map::Static::no_sidewalks(ctx, app)));
                }
                "favorite buildings" => {
                    app.primary.layer = Some(Box::new(favorites::ShowFavorites::new(ctx, app)));
                }
                "pandemic model" => {
                    app.primary.layer = Some(Box::new(pandemic::Pandemic::new(
                        ctx,
                        app,
                        pandemic::Options {
                            heatmap: Some(HeatmapOptions::new()),
                            state: pandemic::SEIR::Infected,
                        },
                    )));
                }
                "blackholes" => {
                    app.primary.layer = Some(Box::new(map::Static::blackholes(ctx, app)));
                }
                "congestion caps" => {
                    app.primary.layer = Some(Box::new(map::CongestionCaps::new(ctx, app)));
                }
                "parking occupancy" => {
                    app.primary.layer = Some(Box::new(parking::Occupancy::new(
                        ctx, app, true, true, true, false, true,
                    )));
                }
                "parking efficiency" => {
                    app.primary.layer = Some(Box::new(parking::Efficiency::new(ctx, app)));
                }
                "population map" => {
                    app.primary.layer = Some(Box::new(population::PopulationMap::new(
                        ctx,
                        app,
                        population::Options {
                            heatmap: Some(HeatmapOptions::new()),
                        },
                    )));
                }
                "throughput" => {
                    app.primary.layer = Some(Box::new(traffic::Throughput::new(
                        ctx,
                        app,
                        AgentType::all().into_iter().collect(),
                    )));
                }
                "traffic jams" => {
                    app.primary.layer = Some(Box::new(traffic::TrafficJams::new(ctx, app)));
                }
                "transit network" => {
                    app.primary.layer = Some(Box::new(transit::TransitNetwork::new(
                        ctx, app, false, true, true,
                    )));
                }
                "traffic signal demand" => {
                    return Transition::Replace(dashboards::TrafficSignalDemand::new(ctx, app));
                }
                "commuter patterns" => {
                    return Transition::Replace(dashboards::CommuterPatterns::new(ctx, app));
                }
                _ => unreachable!(),
            },
            _ => {
                if self.panel.clicked_outside(ctx) {
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
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

/// Creates the top row for any layer panel.
pub fn header(ctx: &mut EventCtx, name: &str) -> Widget {
    Widget::row(vec![
        Widget::draw_svg(ctx, "system/assets/tools/layers.svg").centered_vert(),
        name.draw_text(ctx).centered_vert(),
        ctx.style().btn_close_widget(ctx),
    ])
}

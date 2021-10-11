mod bike_network;
mod explore;
mod labels;
mod layers;
//mod magnifying;
mod predict;
mod quick_sketch;
mod route;

use map_gui::tools::{grey_out_map, open_browser, CityPicker};
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Panel, ScreenDims, SimpleState, State, Text,
    TextExt, VerticalAlignment, Widget,
};

pub use self::explore::ExploreMap;
pub use self::layers::Layers;
use crate::app::{App, Transition};
pub use predict::ModeShiftData;
pub use route::RoutingPreferences;

// The 3 modes are very different States, so TabController doesn't seem like the best fit
#[derive(PartialEq)]
pub enum Tab {
    Explore,
    Create,
    Route,
    PredictImpact,
}

pub trait TakeLayers {
    fn take_layers(self) -> Layers;
}

impl Tab {
    pub fn make_left_panel(self, ctx: &mut EventCtx, app: &App, contents: Widget) -> Panel {
        // Ideally TabController could manage this, but the contents of each section are
        // substantial, controlled by entirely different States.

        let mut contents = Some(contents.section(ctx));

        let mut col = vec![Widget::row(vec![
            ctx.style()
                .btn_plain
                .btn()
                .image_path("system/assets/pregame/logo.svg")
                .image_dims(50.0)
                .build_widget(ctx, "about A/B Street"),
            map_gui::tools::change_map_btn(ctx, app)
                .centered_vert()
                .align_right(),
        ])];

        col.push(
            ctx.style()
                .btn_tab
                .icon_text("system/assets/tools/pan.svg", "Explore")
                .hotkey(Key::Num1)
                .disabled(self == Tab::Explore)
                .build_def(ctx),
        );
        if self == Tab::Explore {
            col.push(contents.take().unwrap());
        }

        col.push(
            ctx.style()
                .btn_tab
                .icon_text("system/assets/tools/pin.svg", "Your Trip")
                .hotkey(Key::Num3)
                .disabled(self == Tab::Route)
                .build_def(ctx),
        );
        if self == Tab::Route {
            col.push(contents.take().unwrap());
        }

        col.push(
            ctx.style()
                .btn_tab
                .icon_text("system/assets/tools/pencil.svg", "Add bike lanes")
                .hotkey(Key::Num2)
                .disabled(self == Tab::Create)
                .build_def(ctx),
        );
        if self == Tab::Create {
            col.push(contents.take().unwrap());
        }

        col.push(
            ctx.style()
                .btn_tab
                .icon_text("system/assets/meters/trip_histogram.svg", "Predict impact")
                .hotkey(Key::Num4)
                .disabled(self == Tab::PredictImpact)
                .build_def(ctx),
        );
        if self == Tab::PredictImpact {
            col.push(contents.take().unwrap());
        }

        let mut panel = Panel::new_builder(Widget::col(col))
            // The different tabs have different widths. To avoid the UI bouncing around as the user
            // navigates, this is hardcoded to be a bit wider than the widest tab.
            .exact_size(ScreenDims {
                width: 620.0,
                height: ctx.canvas.window_height,
            })
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top);
        if self == Tab::Route {
            // Hovering on a card
            panel = panel.ignore_initial_events();
        }
        panel.build(ctx)
    }

    pub fn handle_action<T: TakeLayers + State<App>>(
        self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
    ) -> Option<Transition> {
        match action {
            "about A/B Street" => Some(Transition::Push(About::new_state(ctx))),
            "change map" => {
                Some(Transition::Push(CityPicker::new_state(
                    ctx,
                    app,
                    Box::new(move |ctx, app| {
                        // Since we're totally changing maps, don't reuse the Layers
                        let layers = Layers::new(ctx, app);
                        Transition::Multi(vec![
                            Transition::Pop,
                            Transition::Replace(match self {
                                Tab::Explore => ExploreMap::new_state(ctx, app, layers),
                                Tab::Create => {
                                    quick_sketch::QuickSketch::new_state(ctx, app, layers)
                                }
                                Tab::Route => route::RoutePlanner::new_state(ctx, app, layers),
                                Tab::PredictImpact => {
                                    predict::ShowGaps::new_state(ctx, app, layers)
                                }
                            }),
                        ])
                    }),
                )))
            }
            "Explore" => Some(Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![ExploreMap::new_state(ctx, app, state.take_layers())]
            }))),
            "Your Trip" => Some(Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![route::RoutePlanner::new_state(
                    ctx,
                    app,
                    state.take_layers(),
                )]
            }))),
            "Add bike lanes" => {
                // This is only necessary to do coming from ExploreMap, but eh
                app.primary.current_selection = None;
                Some(Transition::ConsumeState(Box::new(|state, ctx, app| {
                    let state = state.downcast::<T>().ok().unwrap();
                    vec![quick_sketch::QuickSketch::new_state(
                        ctx,
                        app,
                        state.take_layers(),
                    )]
                })))
            }
            "Predict impact" => Some(Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![predict::ShowGaps::new_state(ctx, app, state.take_layers())]
            }))),
            _ => None,
        }
    }
}

struct About;

impl About {
    fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("About A/B Street").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Text::from_multiline(vec![
                Line("Created by Dustin Carlino, Yuwen Li, & Michael Kirk").small(),
                Line("Data from OpenStreetMap, King County GIS, King County LIDAR").small(),
            ])
            .into_widget(ctx),
            "This is a simplified version. Check out the full version below.".text_widget(ctx),
            ctx.style().btn_outline.text("abstreet.org").build_def(ctx),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(About))
    }
}

impl SimpleState<App> for About {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        if x == "close" {
            return Transition::Pop;
        } else if x == "abstreet.org" {
            open_browser("https://abstreet.org");
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
}

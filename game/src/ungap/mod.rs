mod bike_network;
mod explore;
mod labels;
mod layers;
//mod magnifying;
mod predict;
mod quick_sketch;
mod trip;

use geom::CornerRadii;
use map_gui::tools::{grey_out_map, open_browser, CityPicker};
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Panel, ScreenDims, SimpleState, State, Text,
    VerticalAlignment, Widget, DEFAULT_CORNER_RADIUS,
};

pub use self::explore::ExploreMap;
pub use self::layers::Layers;
use crate::app::{App, Transition};
pub use predict::ModeShiftData;
pub use trip::RoutingPreferences;

// The 3 modes are very different States, so TabController doesn't seem like the best fit
#[derive(PartialEq)]
pub enum Tab {
    Explore,
    Trip,
    AddLanes,
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

        let header = Widget::row(vec![
            ctx.style()
                .btn_plain
                .btn()
                .image_path("system/assets/pregame/logo.svg")
                .image_dims(50.0)
                .build_widget(ctx, "about A/B Street"),
            Line("Ungap the Map")
                .small_heading()
                .into_widget(ctx)
                .centered_vert(),
            map_gui::tools::change_map_btn(ctx, app)
                .centered_vert()
                .align_right(),
        ]);

        let mut build_tab = |(tab, image_path, tab_title, hotkey): (Tab, &str, &str, Key)| {
            let mut btn = ctx
                .style()
                .btn_tab
                .icon_text(image_path, tab_title)
                .hotkey(hotkey);

            if self == tab {
                btn = btn
                    .corner_rounding(CornerRadii {
                        top_left: DEFAULT_CORNER_RADIUS,
                        top_right: DEFAULT_CORNER_RADIUS,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    })
                    .disabled(true);
            }

            // Add a little margin to compensate for the border on the tab content
            // otherwise things look ever-so-slightly out of alignment.
            let btn_widget = btn.build_def(ctx).margin_left(1);
            let mut tab_elements = vec![btn_widget];

            if self == tab {
                let mut contents = contents.take().unwrap();
                contents = contents.corner_rounding(CornerRadii {
                    top_left: 0.0,
                    top_right: DEFAULT_CORNER_RADIUS,
                    bottom_left: DEFAULT_CORNER_RADIUS,
                    bottom_right: DEFAULT_CORNER_RADIUS,
                });
                tab_elements.push(contents);
            }

            Widget::custom_col(tab_elements)
        };

        let tabs = Widget::col(vec![
            build_tab((
                Tab::Explore,
                "system/assets/tools/pan.svg",
                "Explore",
                Key::Num1,
            )),
            build_tab((
                Tab::Trip,
                "system/assets/tools/pin.svg",
                "Your Trip",
                Key::Num2,
            )),
            build_tab((
                Tab::AddLanes,
                "system/assets/tools/pencil.svg",
                "Add bike lanes",
                Key::Num3,
            )),
            build_tab((
                Tab::PredictImpact,
                "system/assets/meters/trip_histogram.svg",
                "Predict impact",
                Key::Num4,
            )),
        ]);

        let mut panel = Panel::new_builder(Widget::col(vec![header, tabs]))
            // The different tabs have different widths. To avoid the UI bouncing around as the user
            // navigates, this is hardcoded to be a bit wider than the widest tab.
            .exact_size(ScreenDims {
                width: 620.0,
                height: ctx.canvas.window_height,
            })
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top);
        if self == Tab::Trip {
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
                                Tab::Trip => trip::TripPlanner::new_state(ctx, app, layers),
                                Tab::AddLanes => {
                                    quick_sketch::QuickSketch::new_state(ctx, app, layers)
                                }
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
                vec![trip::TripPlanner::new_state(ctx, app, state.take_layers())]
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
            ctx.style().btn_outline.text("Read more").build_def(ctx),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(About))
    }
}

impl SimpleState<App> for About {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        if x == "close" {
            return Transition::Pop;
        } else if x == "Read more" {
            open_browser("https://a-b-street.github.io/docs/software/ungap_the_map/index.html");
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
}

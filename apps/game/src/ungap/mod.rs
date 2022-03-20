mod bike_network;
mod explore;
mod layers;
mod predict;
mod quick_sketch;
mod trip;

use geom::CornerRadii;
use map_gui::tools::CityPicker;
use widgetry::{
    EventCtx, HorizontalAlignment, Key, Line, Panel, PanelDims, State, VerticalAlignment, Widget,
    DEFAULT_CORNER_RADIUS,
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

        // map_gui::tools::app_header uses 2 rows, but we've tuned the horizontal space here. It's
        // nicer to fit on one row.
        let header = Widget::row(vec![
            map_gui::tools::home_btn(ctx),
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
                "Your trip",
                Key::Num2,
            )),
            build_tab((
                Tab::AddLanes,
                "system/assets/tools/pencil.svg",
                "Propose new bike lanes",
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
            .dims_width(PanelDims::ExactPixels(620.0))
            .dims_height(PanelDims::ExactPercent(1.0))
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
            "Home" => Some(Transition::Pop),
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
            "Your trip" => Some(Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![trip::TripPlanner::new_state(ctx, app, state.take_layers())]
            }))),
            "Propose new bike lanes" => {
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

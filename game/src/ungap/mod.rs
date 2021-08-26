mod bike_network;
mod explore;
mod labels;
mod layers;
//mod magnifying;
mod quick_sketch;
mod route;
mod share;

use map_gui::tools::{grey_out_map, nice_map_name, open_browser, CityPicker};
use widgetry::{
    lctrl, EventCtx, GfxCtx, Key, Line, Panel, SimpleState, State, Text, TextExt, Widget,
};

pub use self::explore::ExploreMap;
pub use self::layers::Layers;
use crate::app::{App, Transition};
pub use share::PROPOSAL_HOST_URL;

// The 3 modes are very different States, so TabController doesn't seem like the best fit
#[derive(PartialEq)]
pub enum Tab {
    Explore,
    Create,
    Route,
}

pub trait TakeLayers {
    fn take_layers(self) -> Layers;
}

impl Tab {
    pub fn make_header(self, ctx: &mut EventCtx, app: &App) -> Widget {
        Widget::col(vec![
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .btn()
                    .image_path("system/assets/pregame/logo.svg")
                    .image_dims(50.0)
                    .build_widget(ctx, "about A/B Street"),
                ctx.style()
                    .btn_popup_icon_text(
                        "system/assets/tools/map.svg",
                        nice_map_name(app.primary.map.get_name()),
                    )
                    .hotkey(lctrl(Key::L))
                    .build_widget(ctx, "change map")
                    .centered_vert()
                    .align_right(),
            ]),
            Widget::row(vec![
                ctx.style()
                    .btn_tab
                    .icon_text("system/assets/tools/pan.svg", "Explore")
                    .hotkey(Key::E)
                    .disabled(self == Tab::Explore)
                    .build_def(ctx),
                ctx.style()
                    .btn_tab
                    .icon_text("system/assets/tools/pencil.svg", "Create new bike lanes")
                    .hotkey(Key::C)
                    .disabled(self == Tab::Create)
                    .build_def(ctx),
                ctx.style()
                    .btn_tab
                    .icon_text("system/assets/tools/pin.svg", "Plan a route")
                    .hotkey(Key::R)
                    .disabled(self == Tab::Route)
                    .build_def(ctx),
            ]),
        ])
    }

    pub fn handle_action<T: TakeLayers + State<App>>(
        self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
    ) -> Transition {
        match action {
            "about A/B Street" => Transition::Push(About::new_state(ctx)),
            "change map" => {
                Transition::Push(CityPicker::new_state(
                    ctx,
                    app,
                    Box::new(|ctx, app| {
                        Transition::Multi(vec![
                            Transition::Pop,
                            // Since we're totally changing maps, don't reuse the Layers
                            // TODO Keep current tab...
                            Transition::Replace(ExploreMap::launch(ctx, app)),
                        ])
                    }),
                ))
            }
            "Explore" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![ExploreMap::new_state(ctx, app, state.take_layers())]
            })),
            "Create new bike lanes" => {
                // This is only necessary to do coming from ExploreMap, but eh
                app.primary.current_selection = None;
                Transition::ConsumeState(Box::new(|state, ctx, app| {
                    let state = state.downcast::<T>().ok().unwrap();
                    vec![quick_sketch::QuickSketch::new_state(
                        ctx,
                        app,
                        state.take_layers(),
                    )]
                }))
            }
            "Plan a route" => Transition::ConsumeState(Box::new(|state, ctx, app| {
                let state = state.downcast::<T>().ok().unwrap();
                vec![route::RoutePlanner::new_state(
                    ctx,
                    app,
                    state.take_layers(),
                )]
            })),
            x => panic!("Unhandled action {}", x),
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

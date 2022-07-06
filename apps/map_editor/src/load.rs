use abstio::{Manifest, MapName};
use raw_map::RawMap;
use widgetry::tools::{FileLoader, PopupMsg, URLManager};
use widgetry::{
    Autocomplete, EventCtx, GfxCtx, Image, Line, Outcome, Panel, State, Transition, Widget,
};

use crate::app::App;
use crate::camera::CameraState;

pub struct PickMap {
    panel: Panel,
}

impl PickMap {
    pub fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let mut entries = Vec::new();
        for name in MapName::list_all_maps_merged(&Manifest::load()) {
            entries.push((name.describe(), abstio::path_raw_map(&name)));
        }

        Box::new(PickMap {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a map").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    Image::from_path("system/assets/tools/search.svg").into_widget(ctx),
                    Autocomplete::new_widget(ctx, entries, 20).named("search"),
                ]),
            ]))
            .exact_size_percent(80, 80)
            .build(ctx),
        })
    }
}

impl State<App> for PickMap {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            }
        }
        if let Some(mut paths) = self.panel.autocomplete_done::<String>("search") {
            if !paths.is_empty() {
                return Transition::Push(load_map(
                    ctx,
                    paths.remove(0),
                    app.model.include_bldgs,
                    None,
                ));
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}

pub fn load_map(
    ctx: &mut EventCtx,
    path: String,
    include_bldgs: bool,
    center_camera: Option<String>,
) -> Box<dyn State<App>> {
    FileLoader::<App, RawMap>::new_state(
        ctx,
        path,
        Box::new(move |ctx, app, timer, map| match map {
            Ok(map) => {
                app.model = crate::model::Model::from_map(ctx, map, include_bldgs, timer);

                if !URLManager::change_camera(
                    ctx,
                    center_camera.as_ref(),
                    &app.model.map.streets.gps_bounds,
                ) && !app.model.map.name.map.is_empty()
                {
                    CameraState::load(ctx, &app.model.map.name);
                }

                Transition::Clear(vec![crate::app::MainState::new_state(ctx, app)])
            }
            Err(err) => Transition::Replace(PopupMsg::new_state(
                ctx,
                "Error",
                vec![
                    "The format of this file has become out-of-sync with this version of the code."
                        .to_string(),
                    "Please file an issue and ask for an update. Sorry for the hassle!".to_string(),
                    format!("Error: {}", err),
                ],
            )),
        }),
    )
}

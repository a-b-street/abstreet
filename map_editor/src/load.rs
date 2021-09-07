use abstio::{Manifest, MapName};
use geom::Percent;
use map_gui::load::FileLoader;
use map_gui::tools::PopupMsg;
use map_model::raw::RawMap;
use widgetry::{
    Autocomplete, EventCtx, GfxCtx, Image, Line, Outcome, Panel, State, Transition, Widget,
};

use crate::app::App;

pub struct PickMap {
    panel: Panel,
}

impl PickMap {
    pub fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let mut autocomplete_entries = Vec::new();
        let mut buttons = Vec::new();

        for name in MapName::list_all_maps_merged(&Manifest::load()) {
            let path = abstio::path_raw_map(&name);
            buttons.push(
                ctx.style()
                    .btn_outline
                    .text(name.describe())
                    .build_widget(ctx, &path)
                    .margin_right(10)
                    .margin_below(10),
            );
            autocomplete_entries.push((name.describe(), path));
        }

        Box::new(PickMap {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a map").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    Image::from_path("system/assets/tools/search.svg").into_widget(ctx),
                    Autocomplete::new_widget(ctx, autocomplete_entries).named("search"),
                ])
                .padding(8),
                Widget::custom_row(buttons).flex_wrap(ctx, Percent::int(70)),
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
                _ => {
                    return Transition::Push(load_map(ctx, x, app.model.include_bldgs));
                }
            }
        }
        if let Some(mut paths) = self.panel.autocomplete_done::<String>("search") {
            if !paths.is_empty() {
                return Transition::Push(load_map(ctx, paths.remove(0), app.model.include_bldgs));
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}

pub fn load_map(ctx: &mut EventCtx, path: String, include_bldgs: bool) -> Box<dyn State<App>> {
    FileLoader::<App, RawMap>::new_state(
        ctx,
        path,
        Box::new(move |ctx, app, timer, map| match map {
            Ok(map) => {
                app.model = crate::model::Model::from_map(ctx, map, include_bldgs, timer);
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

use abstio::{Manifest, MapName};
use geom::Percent;
use map_gui::load::FileLoader;
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
            buttons.push(
                ctx.style()
                    .btn_outline
                    .text(name.describe())
                    .build_widget(ctx, &name.path())
                    .margin_right(10)
                    .margin_below(10),
            );
            autocomplete_entries.push((name.describe(), name.path()));
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
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition<App> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                path => {
                    return load_map(ctx, MapName::from_path(path).unwrap());
                }
            }
        }
        if let Some(mut paths) = self.panel.autocomplete_done::<String>("search") {
            if !paths.is_empty() {
                return load_map(ctx, MapName::from_path(&paths.remove(0)).unwrap());
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}

fn load_map(ctx: &mut EventCtx, map: MapName) -> Transition<App> {
    Transition::Push(FileLoader::<App, RawMap>::new_state(
        ctx,
        abstio::path_raw_map(&map),
        Box::new(|ctx, app, timer, map| {
            // TODO Handle corrupt files -- which especially might happen on the web!
            let map = map.unwrap();
            let include_bldgs = false;
            app.model = crate::model::Model::from_map(ctx, map, include_bldgs, timer);
            Transition::Clear(vec![crate::app::MainState::new_state(ctx, app)])
        }),
    ))
}

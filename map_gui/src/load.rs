use abstio::MapName;
use widgetry::tools::PopupMsg;
use widgetry::tools::{FileLoader, RawBytes};
use widgetry::{EventCtx, GfxCtx, State, Transition};

use crate::AppLike;

pub struct MapLoader;

impl MapLoader {
    pub fn new_state<A: AppLike + 'static>(
        ctx: &mut EventCtx,
        app: &A,
        name: MapName,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        if app.map().get_name() == &name {
            return Box::new(MapAlreadyLoaded {
                on_load: Some(on_load),
            });
        }

        MapLoader::force_reload(ctx, name, on_load)
    }

    /// Even if the current map name matches, still reload.
    pub fn force_reload<A: AppLike + 'static>(
        ctx: &mut EventCtx,
        name: MapName,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        // TODO Generalize this more, maybe with some kind of country code -> font config
        if let Some(extra_font) = match name.city.country.as_ref() {
            "il" => Some("NotoSansHebrew-Regular.ttf"),
            "ir" | "ly" => Some("NotoSansArabic-Regular.ttf"),
            "jp" => Some("NotoSerifCJKtc-Regular.otf"),
            "tw" => Some("NotoSerifCJKtc-Regular.otf"),
            _ => None,
        } {
            if !ctx.is_font_loaded(extra_font) {
                return FileLoader::<A, RawBytes>::new_state(
                    ctx,
                    abstio::path(format!("system/extra_fonts/{}", extra_font)),
                    Box::new(move |ctx, app, _, bytes| match bytes {
                        Ok(bytes) => {
                            ctx.load_font(extra_font, bytes.0);
                            Transition::Replace(MapLoader::new_state(ctx, app, name, on_load))
                        }
                        Err(err) => Transition::Replace(PopupMsg::new_state(
                            ctx,
                            "Error",
                            vec![format!("Couldn't load {}", extra_font), err.to_string()],
                        )),
                    }),
                );
            }
        }

        FileLoader::<A, map_model::Map>::new_state(
            ctx,
            name.path(),
            Box::new(move |ctx, app, timer, map| {
                match map {
                    Ok(mut map) => {
                        // Kind of a hack. We can't generically call Map::new with the FileLoader.
                        map.map_loaded_directly(timer);

                        app.map_switched(ctx, map, timer);

                        (on_load)(ctx, app)
                    }
                    Err(err) => Transition::Replace(PopupMsg::new_state(
                        ctx,
                        "Error",
                        vec![
                            format!("Couldn't load {}", name.describe()),
                            err.to_string(),
                        ],
                    )),
                }
            }),
        )
    }
}

struct MapAlreadyLoaded<A: AppLike> {
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}
impl<A: AppLike + 'static> State<A> for MapAlreadyLoaded<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        (self.on_load.take().unwrap())(ctx, app)
    }
    fn draw(&self, _: &mut GfxCtx, _: &A) {}
}

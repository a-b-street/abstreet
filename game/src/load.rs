//! Loading large resources (like maps, scenarios, and prebaked data) requires different strategies
//! on native and web. Both cases are wrapped up as a State that runs a callback when done.

use map_model::Map;
use sim::Sim;
use widgetry::{Color, EventCtx, GfxCtx};

use crate::app::App;
use crate::game::{State, Transition};

#[cfg(not(target_arch = "wasm32"))]
pub use native_loader::MapLoader;

#[cfg(target_arch = "wasm32")]
pub use wasm_loader::MapLoader;

struct MapAlreadyLoaded {
    on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
}
impl State for MapAlreadyLoaded {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        (self.on_load)(ctx, app)
    }
    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

#[cfg(not(target_arch = "wasm32"))]
mod native_loader {
    use super::*;

    pub struct MapLoader {
        name: String,
        on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
    }

    impl MapLoader {
        pub fn new(
            _: &mut EventCtx,
            app: &App,
            name: String,
            on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
        ) -> Box<dyn State> {
            if app.primary.map.get_name() == &name {
                return Box::new(MapAlreadyLoaded { on_load });
            }

            Box::new(MapLoader { name, on_load })
        }
    }

    impl State for MapLoader {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
            if abstutil::file_exists(abstutil::path_map(&self.name)) {
                ctx.loading_screen("load map", |ctx, mut timer| {
                    let map = Map::new(abstutil::path_map(&self.name), timer);
                    let sim = Sim::new(
                        &map,
                        app.primary.current_flags.sim_flags.opts.clone(),
                        &mut timer,
                    );
                    app.map_switched(ctx, map, sim, timer);
                });
                (self.on_load)(ctx, app)
            } else {
                // TODO Some kind of UI for running the updater from here!
                Transition::Replace(crate::game::PopupMsg::new(
                    ctx,
                    "Missing data",
                    vec![
                        format!("{} is missing", abstutil::path_map(&self.name)),
                        "You need to opt into this by modifying data/config and running the \
                         updater"
                            .to_string(),
                    ],
                ))
            }
        }

        fn draw(&self, g: &mut GfxCtx, _: &App) {
            g.clear(Color::BLACK);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_loader {
    use futures_channel::oneshot;
    use instant::Instant;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    use abstutil::Timer;
    use geom::Duration;
    use widgetry::{Line, Panel, Text, UpdateType};

    use super::*;

    // Instead of blockingly reading a file within ctx.loading_screen, on the web have to
    // asynchronously make an HTTP request and keep "polling" for completion in a way that's
    // compatible with winit's event loop.
    pub struct MapLoader {
        response: oneshot::Receiver<Vec<u8>>,
        on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
        panel: Panel,
        started: Instant,
        url: String,
    }

    impl MapLoader {
        pub fn new(
            ctx: &mut EventCtx,
            app: &App,
            name: String,
            on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
        ) -> Box<dyn State> {
            if app.primary.map.get_name() == &name {
                return Box::new(MapAlreadyLoaded { on_load });
            }
            // TODO If we want to load montlake, just pull from bundled data.

            let url = if cfg!(feature = "wasm_s3") {
                format!(
                    "http://abstreet.s3-website.us-east-2.amazonaws.com/system/maps/{}.bin",
                    name
                )
            } else {
                format!("http://0.0.0.0:8000/system/maps/{}.bin", name)
            };

            // Make the HTTP request nonblockingly. When the response is received, send it through
            // the channel.
            let (tx, rx) = oneshot::channel();
            let url_copy = url.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut opts = RequestInit::new();
                opts.method("GET");
                opts.mode(RequestMode::Cors);
                let request = Request::new_with_str_and_init(&url_copy, &opts).unwrap();

                let window = web_sys::window().unwrap();
                let resp_value = JsFuture::from(window.fetch_with_request(&request))
                    .await
                    .unwrap();
                let resp: Response = resp_value.dyn_into().unwrap();
                let buf = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap();
                let array = js_sys::Uint8Array::new(&buf);
                let bytes = array.to_vec();
                tx.send(bytes).unwrap();
            });

            Box::new(MapLoader {
                response: rx,
                on_load,
                panel: ctx.make_loading_screen(Text::from(Line(format!("Loading {}...", url)))),
                started: Instant::now(),
                url,
            })
        }
    }

    impl State for MapLoader {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
            if let Some(resp) = self.response.try_recv().unwrap() {
                // TODO We stop drawing and start blocking at this point. It can take a
                // while. Any way to make it still be nonblockingish? Maybe put some of the work
                // inside that spawn_local?

                let mut timer = Timer::new("finish loading map");
                let map: Map = abstutil::from_binary(&resp).unwrap();
                let sim = Sim::new(
                    &map,
                    app.primary.current_flags.sim_flags.opts.clone(),
                    &mut timer,
                );
                app.map_switched(ctx, map, sim, &mut timer);

                return (self.on_load)(ctx, app);
            }

            self.panel = ctx.make_loading_screen(Text::from_multiline(vec![
                Line(format!("Loading {}...", self.url)),
                Line(format!(
                    "Time spent: {}",
                    Duration::realtime_elapsed(self.started)
                )),
            ]));

            // Until the response is received, just ask winit to regularly call event(), so we can
            // keep polling the channel.
            ctx.request_update(UpdateType::Game);
            Transition::Keep
        }

        fn draw(&self, g: &mut GfxCtx, _: &App) {
            // TODO Progress bar for bytes received
            g.clear(Color::BLACK);
            self.panel.draw(g);
        }
    }
}

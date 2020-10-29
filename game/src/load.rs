//! Loading large resources (like maps, scenarios, and prebaked data) requires different strategies
//! on native and web. Both cases are wrapped up as a State that runs a callback when done.

use serde::de::DeserializeOwned;

use abstutil::Timer;
use sim::Sim;
use widgetry::{Color, EventCtx, GfxCtx, State};

use crate::app::App;
use crate::game::{PopupMsg, Transition};

#[cfg(not(target_arch = "wasm32"))]
pub use native_loader::FileLoader;

#[cfg(target_arch = "wasm32")]
pub use wasm_loader::FileLoader;

pub struct MapLoader;

impl MapLoader {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        name: String,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State<App>> {
        if app.primary.map.get_name() == &name {
            return Box::new(MapAlreadyLoaded {
                on_load: Some(on_load),
            });
        }

        // TODO If we want to load montlake on the web, just pull from bundled data.
        FileLoader::<map_model::Map>::new(
            ctx,
            abstutil::path_map(&name),
            Box::new(move |ctx, app, timer, map| {
                match map {
                    Ok(mut map) => {
                        // Kind of a hack. We can't generically call Map::new with the FileLoader.
                        map.map_loaded_directly();

                        let sim = Sim::new(
                            &map,
                            app.primary.current_flags.sim_flags.opts.clone(),
                            timer,
                        );
                        app.map_switched(ctx, map, sim, timer);

                        (on_load)(ctx, app)
                    }
                    Err(err) => Transition::Replace(PopupMsg::new(
                        ctx,
                        "Error",
                        vec![format!("Couldn't load {}", name), err],
                    )),
                }
            }),
        )
    }
}

struct MapAlreadyLoaded {
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut App) -> Transition>>,
}
impl State<App> for MapAlreadyLoaded {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        (self.on_load.take().unwrap())(ctx, app)
    }
    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

#[cfg(not(target_arch = "wasm32"))]
mod native_loader {
    use super::*;

    pub struct FileLoader<T> {
        path: String,
        // Wrapped in an Option just to make calling from event() work. Technically this is unsafe
        // if a caller fails to pop the FileLoader state in their transitions!
        on_load: Option<
            Box<dyn FnOnce(&mut EventCtx, &mut App, &mut Timer, Result<T, String>) -> Transition>,
        >,
    }

    impl<T: 'static + DeserializeOwned> FileLoader<T> {
        pub fn new(
            _: &mut EventCtx,
            path: String,
            on_load: Box<
                dyn FnOnce(&mut EventCtx, &mut App, &mut Timer, Result<T, String>) -> Transition,
            >,
        ) -> Box<dyn State<App>> {
            Box::new(FileLoader {
                path,
                on_load: Some(on_load),
            })
        }
    }

    impl<T: 'static + DeserializeOwned> State<App> for FileLoader<T> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
            ctx.loading_screen(format!("load {}", self.path), |ctx, timer| {
                // Assumes a binary file
                let file = abstutil::maybe_read_binary(self.path.clone(), timer)
                    .map_err(|err| err.to_string());
                (self.on_load.take().unwrap())(ctx, app, timer, file)
            })
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

    use geom::Duration;
    use widgetry::{Line, Panel, State, Text, UpdateType};

    use super::*;

    // Instead of blockingly reading a file within ctx.loading_screen, on the web have to
    // asynchronously make an HTTP request and keep "polling" for completion in a way that's
    // compatible with winit's event loop.
    pub struct FileLoader<T> {
        response: oneshot::Receiver<Result<Vec<u8>, String>>,
        on_load: Option<
            Box<dyn FnOnce(&mut EventCtx, &mut App, &mut Timer, Result<T, String>) -> Transition>,
        >,
        panel: Panel,
        started: Instant,
        url: String,
    }

    impl<T: 'static + DeserializeOwned> FileLoader<T> {
        pub fn new(
            ctx: &mut EventCtx,
            path: String,
            on_load: Box<
                dyn FnOnce(&mut EventCtx, &mut App, &mut Timer, Result<T, String>) -> Transition,
            >,
        ) -> Box<dyn State<App>> {
            let url = if cfg!(feature = "wasm_s3") {
                format!(
                    "http://abstreet.s3-website.us-east-2.amazonaws.com/{}",
                    path.strip_prefix(&abstutil::path("")).unwrap()
                )
            } else {
                format!(
                    "http://0.0.0.0:8000/{}",
                    path.strip_prefix(&abstutil::path("")).unwrap()
                )
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
                match JsFuture::from(window.fetch_with_request(&request)).await {
                    Ok(resp_value) => {
                        let resp: Response = resp_value.dyn_into().unwrap();
                        let buf = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap();
                        let array = js_sys::Uint8Array::new(&buf);
                        tx.send(Ok(array.to_vec())).unwrap();
                    }
                    Err(err) => {
                        tx.send(Err(format!("{:?}", err))).unwrap();
                    }
                }
            });

            Box::new(FileLoader {
                response: rx,
                on_load: Some(on_load),
                panel: ctx.make_loading_screen(Text::from(Line(format!("Loading {}...", url)))),
                started: Instant::now(),
                url,
            })
        }
    }

    impl<T: 'static + DeserializeOwned> State<App> for FileLoader<T> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
            if let Some(maybe_resp) = self.response.try_recv().unwrap() {
                // TODO We stop drawing and start blocking at this point. It can take a
                // while. Any way to make it still be nonblockingish? Maybe put some of the work
                // inside that spawn_local?
                let mut timer = Timer::new(format!("Loading {}...", self.url));
                let result = maybe_resp
                    .and_then(|resp| abstutil::from_binary(&resp).map_err(|err| err.to_string()));
                return (self.on_load.take().unwrap())(ctx, app, &mut timer, result);
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

//! Loading large resources (like maps, scenarios, and prebaked data) requires different strategies
//! on native and web. Both cases are wrapped up as a State that runs a callback when done.

use serde::de::DeserializeOwned;

use abstutil::{MapName, Timer};
use widgetry::{Color, EventCtx, GfxCtx, State, Transition};

use crate::game::PopupMsg;
use crate::AppLike;

#[cfg(not(target_arch = "wasm32"))]
pub use native_loader::FileLoader;

#[cfg(target_arch = "wasm32")]
pub use wasm_loader::FileLoader;

pub struct MapLoader;

impl MapLoader {
    pub fn new<A: AppLike + 'static>(
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

        // TODO If we want to load montlake on the web, just pull from bundled data.
        FileLoader::<A, map_model::Map>::new(
            ctx,
            name.path(),
            Box::new(move |ctx, app, timer, map| {
                match map {
                    Ok(mut map) => {
                        // Kind of a hack. We can't generically call Map::new with the FileLoader.
                        map.map_loaded_directly();

                        app.map_switched(ctx, map, timer);

                        (on_load)(ctx, app)
                    }
                    Err(err) => Transition::Replace(PopupMsg::new(
                        ctx,
                        "Error",
                        vec![format!("Couldn't load {}", name.describe()), err],
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

#[cfg(not(target_arch = "wasm32"))]
mod native_loader {
    use super::*;

    pub struct FileLoader<A: AppLike, T> {
        path: String,
        // Wrapped in an Option just to make calling from event() work. Technically this is unsafe
        // if a caller fails to pop the FileLoader state in their transitions!
        on_load: Option<
            Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T, String>) -> Transition<A>>,
        >,
    }

    impl<A: AppLike + 'static, T: 'static + DeserializeOwned> FileLoader<A, T> {
        pub fn new(
            _: &mut EventCtx,
            path: String,
            on_load: Box<
                dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T, String>) -> Transition<A>,
            >,
        ) -> Box<dyn State<A>> {
            Box::new(FileLoader {
                path,
                on_load: Some(on_load),
            })
        }
    }

    impl<A: AppLike + 'static, T: 'static + DeserializeOwned> State<A> for FileLoader<A, T> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
            ctx.loading_screen(format!("load {}", self.path), |ctx, timer| {
                let file = if self.path.ends_with(".bin") {
                    abstutil::maybe_read_binary(self.path.clone(), timer)
                } else {
                    abstutil::maybe_read_json(self.path.clone(), timer)
                }
                .map_err(|err| err.to_string());
                (self.on_load.take().unwrap())(ctx, app, timer, file)
            })
        }

        fn draw(&self, g: &mut GfxCtx, _: &A) {
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
    pub struct FileLoader<A: AppLike, T> {
        response: oneshot::Receiver<Result<Vec<u8>, String>>,
        on_load: Option<
            Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T, String>) -> Transition<A>>,
        >,
        panel: Panel,
        started: Instant,
        url: String,
    }

    impl<A: AppLike + 'static, T: 'static + DeserializeOwned> FileLoader<A, T> {
        pub fn new(
            ctx: &mut EventCtx,
            path: String,
            on_load: Box<
                dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T, String>) -> Transition<A>,
            >,
        ) -> Box<dyn State<A>> {
            // Note that files are only gzipepd on S3. When running locally, we just symlink the
            // data/ directory, where files aren't compressed.
            let url = if cfg!(feature = "wasm_s3") {
                // Anytime data with a new binary format is uploaded, the web client has to be
                // re-deployed too
                format!(
                    "http://abstreet.s3-website.us-east-2.amazonaws.com/dev/data/{}.gz",
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
                        if resp.ok() {
                            let buf = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap();
                            let array = js_sys::Uint8Array::new(&buf);
                            tx.send(Ok(array.to_vec())).unwrap();
                        } else {
                            let status = resp.status();
                            let err = resp.status_text();
                            tx.send(Err(format!("HTTP {}: {}", status, err))).unwrap();
                        }
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

    impl<A: AppLike + 'static, T: 'static + DeserializeOwned> State<A> for FileLoader<A, T> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
            if let Some(maybe_resp) = self.response.try_recv().unwrap() {
                // TODO We stop drawing and start blocking at this point. It can take a
                // while. Any way to make it still be nonblockingish? Maybe put some of the work
                // inside that spawn_local?
                let mut timer = Timer::new(format!("Loading {}...", self.url));
                let result = maybe_resp.and_then(|resp| {
                    if self.url.ends_with(".gz") {
                        let decoder = flate2::read::GzDecoder::new(&resp[..]);
                        if self.url.ends_with(".bin.gz") {
                            abstutil::from_binary_reader(decoder)
                        } else {
                            abstutil::from_json_reader(decoder)
                        }
                    } else if self.url.ends_with(".bin") {
                        abstutil::from_binary(&&resp).map_err(|err| err.to_string())
                    } else {
                        abstutil::from_json(&&resp).map_err(|err| err.to_string())
                    }
                });
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

        fn draw(&self, g: &mut GfxCtx, _: &A) {
            // TODO Progress bar for bytes received
            g.clear(Color::BLACK);
            self.panel.draw(g);
        }
    }
}

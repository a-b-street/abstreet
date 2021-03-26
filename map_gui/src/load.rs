//! Loading large resources (like maps, scenarios, and prebaked data) requires different strategies
//! on native and web. Both cases are wrapped up as a State that runs a callback when done.

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use futures_channel::oneshot;
use instant::Instant;
use serde::de::DeserializeOwned;
#[cfg(not(target_arch = "wasm32"))]
use tokio::runtime::Runtime;

use abstio::MapName;
use abstutil::Timer;
use geom::Duration;
use widgetry::{Color, EventCtx, GfxCtx, Line, Panel, State, Text, Transition, UpdateType};

use crate::tools::PopupMsg;
use crate::AppLike;

#[cfg(not(target_arch = "wasm32"))]
pub use native_loader::{FileLoader, RawFileLoader};

#[cfg(target_arch = "wasm32")]
pub use wasm_loader::{FileLoader, RawFileLoader};

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

        // TODO Generalize this more, maybe with some kind of country code -> font config
        if let Some(extra_font) = match name.city.country.as_ref() {
            "ly" => Some("NotoSansArabic-Regular.ttf"),
            "tw" => Some("NotoSerifCJKtc-Regular.otf"),
            _ => None,
        } {
            if !ctx.is_font_loaded(extra_font) {
                return RawFileLoader::<A>::new(
                    ctx,
                    abstio::path(format!("system/extra_fonts/{}", extra_font)),
                    Box::new(move |ctx, app, bytes| match bytes {
                        Ok(bytes) => {
                            ctx.load_font(extra_font, bytes);
                            Transition::Replace(MapLoader::new(ctx, app, name, on_load))
                        }
                        Err(err) => Transition::Replace(PopupMsg::new(
                            ctx,
                            "Error",
                            vec![format!("Couldn't load {}", extra_font), err.to_string()],
                        )),
                    }),
                );
            }
        }

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

#[cfg(not(target_arch = "wasm32"))]
mod native_loader {
    use super::*;

    // This loads a JSON or bincoded file, then deserializes it
    pub struct FileLoader<A: AppLike, T> {
        path: String,
        // Wrapped in an Option just to make calling from event() work. Technically this is unsafe
        // if a caller fails to pop the FileLoader state in their transitions!
        on_load:
            Option<Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T>) -> Transition<A>>>,
    }

    impl<A: AppLike + 'static, T: 'static + DeserializeOwned> FileLoader<A, T> {
        pub fn new(
            _: &mut EventCtx,
            path: String,
            on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T>) -> Transition<A>>,
        ) -> Box<dyn State<A>> {
            Box::new(FileLoader {
                path,
                on_load: Some(on_load),
            })
        }
    }

    impl<A: AppLike + 'static, T: 'static + DeserializeOwned> State<A> for FileLoader<A, T> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
            debug!("Loading {}", self.path);
            ctx.loading_screen(format!("load {}", self.path), |ctx, timer| {
                let file = abstio::read_object(self.path.clone(), timer);
                (self.on_load.take().unwrap())(ctx, app, timer, file)
            })
        }

        fn draw(&self, g: &mut GfxCtx, _: &A) {
            g.clear(Color::BLACK);
        }
    }

    // TODO Ideally merge with FileLoader
    pub struct RawFileLoader<A: AppLike> {
        path: String,
        // Wrapped in an Option just to make calling from event() work. Technically this is unsafe
        // if a caller fails to pop the FileLoader state in their transitions!
        on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A, Result<Vec<u8>>) -> Transition<A>>>,
    }

    impl<A: AppLike + 'static> RawFileLoader<A> {
        pub fn new(
            _: &mut EventCtx,
            path: String,
            on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, Result<Vec<u8>>) -> Transition<A>>,
        ) -> Box<dyn State<A>> {
            Box::new(RawFileLoader {
                path,
                on_load: Some(on_load),
            })
        }
    }

    impl<A: AppLike + 'static> State<A> for RawFileLoader<A> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
            debug!("Loading {}", self.path);
            let bytes = abstio::slurp_file(&self.path);
            (self.on_load.take().unwrap())(ctx, app, bytes)
        }

        fn draw(&self, g: &mut GfxCtx, _: &A) {
            g.clear(Color::BLACK);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_loader {
    use std::io::Read;

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
        response: oneshot::Receiver<Result<Vec<u8>>>,
        on_load:
            Option<Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T>) -> Transition<A>>>,
        panel: Panel,
        started: Instant,
        url: String,
    }

    impl<A: AppLike + 'static, T: 'static + DeserializeOwned> FileLoader<A, T> {
        pub fn new(
            ctx: &mut EventCtx,
            path: String,
            on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T>) -> Transition<A>>,
        ) -> Box<dyn State<A>> {
            // The current URL is of the index.html page. We can find the data directory relative
            // to that.
            let base_url = ctx
                .prerender
                .assets_base_url()
                .expect("assets_base_url should be specified for wasm builds via `Settings`");
            let file_path = path.strip_prefix(&abstio::path("")).unwrap();
            // Note that files are gzipped on S3 and other deployments. When running locally, we
            // just symlink the data/ directory, where files aren't compressed.
            let url =
                if base_url.contains("http://0.0.0.0") || base_url.contains("http://localhost") {
                    format!("{}/{}", base_url, file_path)
                } else if base_url.contains("abstreet.s3-website") {
                    // The directory structure on S3 is a little weird -- the base directory has
                    // data/ alongside game/, fifteen_min/, etc.
                    format!("{}/../data/{}.gz", base_url, file_path)
                } else {
                    format!("{}/{}.gz", base_url, file_path)
                };

            // Make the HTTP request nonblockingly. When the response is received, send it through
            // the channel.
            let (tx, rx) = oneshot::channel();
            let url_copy = url.clone();
            debug!("Loading {}", url_copy);
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
                            tx.send(Err(anyhow!("HTTP {}: {}", status, err))).unwrap();
                        }
                    }
                    Err(err) => {
                        tx.send(Err(anyhow!("{:?}", err))).unwrap();
                    }
                }
            });

            Box::new(FileLoader {
                response: rx,
                on_load: Some(on_load),
                panel: ctx.make_loading_screen(Text::from(format!("Loading {}...", url))),
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
                        abstutil::from_binary(&&resp)
                    } else {
                        abstutil::from_json(&&resp)
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

    // TODO This is a horrible copy of FileLoader. Make the serde FileLoader just build on top of
    // this one!!!
    pub struct RawFileLoader<A: AppLike> {
        response: oneshot::Receiver<Result<Vec<u8>>>,
        on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A, Result<Vec<u8>>) -> Transition<A>>>,
        panel: Panel,
        started: Instant,
        url: String,
    }

    impl<A: AppLike + 'static> RawFileLoader<A> {
        pub fn new(
            ctx: &mut EventCtx,
            path: String,
            on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, Result<Vec<u8>>) -> Transition<A>>,
        ) -> Box<dyn State<A>> {
            // The current URL is of the index.html page. We can find the data directory relative
            // to that.
            let base_url = get_base_url().unwrap();
            let file_path = path.strip_prefix(&abstio::path("")).unwrap();
            // Note that files are gzipped on S3 and other deployments. When running locally, we
            // just symlink the data/ directory, where files aren't compressed.
            let url =
                if base_url.contains("http://0.0.0.0") || base_url.contains("http://localhost") {
                    format!("{}/{}", base_url, file_path)
                } else if base_url.contains("abstreet.s3-website") {
                    // The directory structure on S3 is a little weird -- the base directory has
                    // data/ alongside game/, fifteen_min/, etc.
                    format!("{}/../data/{}.gz", base_url, file_path)
                } else {
                    format!("{}/{}.gz", base_url, file_path)
                };

            // Make the HTTP request nonblockingly. When the response is received, send it through
            // the channel.
            let (tx, rx) = oneshot::channel();
            let url_copy = url.clone();
            debug!("Loading {}", url_copy);
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
                            tx.send(Err(anyhow!("HTTP {}: {}", status, err))).unwrap();
                        }
                    }
                    Err(err) => {
                        tx.send(Err(anyhow!("{:?}", err))).unwrap();
                    }
                }
            });

            Box::new(RawFileLoader {
                response: rx,
                on_load: Some(on_load),
                panel: ctx.make_loading_screen(Text::from(format!("Loading {}...", url))),
                started: Instant::now(),
                url,
            })
        }
    }

    impl<A: AppLike + 'static> State<A> for RawFileLoader<A> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
            if let Some(maybe_resp) = self.response.try_recv().unwrap() {
                let bytes = if self.url.ends_with(".gz") {
                    maybe_resp.and_then(|gzipped| {
                        let mut decoder = flate2::read::GzDecoder::new(&gzipped[..]);
                        let mut buffer: Vec<u8> = Vec::new();
                        decoder
                            .read_to_end(&mut buffer)
                            .map(|_| buffer)
                            .map_err(|err| err.into())
                    })
                } else {
                    maybe_resp
                };
                return (self.on_load.take().unwrap())(ctx, app, bytes);
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

    /// Returns the base URL where the game is running, excluding query parameters and the
    /// implicit index.html that might be there.
    fn get_base_url() -> Result<String> {
        let window = web_sys::window().ok_or(anyhow!("no window?"))?;
        let url = window.location().href().map_err(|err| {
            anyhow!(err
                .as_string()
                .unwrap_or("window.location.href failed".to_string()))
        })?;
        // Consider using a proper url parsing crate. This works fine for now, though.
        let url = url.split("?").next().ok_or(anyhow!("empty URL?"))?;
        Ok(url
            .trim_end_matches("index.html")
            // TODO This is brittle; we should strip off the trailing filename no matter what it
            // is.
            .trim_end_matches("prefetch.html")
            .trim_end_matches("/")
            .to_string())
    }
}

pub struct FutureLoader<A, T>
where
    A: AppLike,
{
    loading_title: String,
    started: Instant,
    panel: Panel,
    receiver: oneshot::Receiver<Result<Box<dyn Send + FnOnce(&A) -> T>>>,
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A, Result<T>) -> Transition<A>>>,

    // If Runtime is dropped, any active tasks will be canceled, so we retain it here even
    // though we never access it. It might make more sense for Runtime to live on App if we're
    // going to be doing more background spawning.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    runtime: Runtime,
}

impl<A, T> FutureLoader<A, T>
where
    A: 'static + AppLike,
    T: 'static,
{
    #[cfg(target_arch = "wasm32")]
    pub fn new(
        ctx: &mut EventCtx,
        future: Pin<Box<dyn Future<Output = Result<Box<dyn Send + FnOnce(&A) -> T>>>>>,
        loading_title: &str,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, Result<T>) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let (tx, receiver) = oneshot::channel();
        wasm_bindgen_futures::spawn_local(async move {
            tx.send(future.await).ok().unwrap();
        });
        Box::new(FutureLoader {
            loading_title: loading_title.to_string(),
            started: Instant::now(),
            panel: ctx.make_loading_screen(Text::from(loading_title)),
            receiver,
            on_load: Some(on_load),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(
        ctx: &mut EventCtx,
        future: Pin<Box<dyn Send + Future<Output = Result<Box<dyn Send + FnOnce(&A) -> T>>>>>,
        loading_title: &str,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, Result<T>) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let runtime = Runtime::new().unwrap();
        let (tx, receiver) = oneshot::channel();
        runtime.spawn(async move {
            tx.send(future.await).ok().unwrap();
        });

        Box::new(FutureLoader {
            loading_title: loading_title.to_string(),
            started: Instant::now(),
            panel: ctx.make_loading_screen(Text::from(loading_title)),
            receiver,
            on_load: Some(on_load),
            runtime,
        })
    }
}

impl<A, T> State<A> for FutureLoader<A, T>
where
    A: 'static + AppLike,
    T: 'static,
{
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        match self.receiver.try_recv() {
            Err(e) => {
                error!("channel failed: {:?}", e);
                let on_load = self.on_load.take().unwrap();
                return on_load(ctx, app, Err(anyhow!("channel canceled")));
            }
            Ok(None) => {
                self.panel = ctx.make_loading_screen(Text::from_multiline(vec![
                    Line(&self.loading_title),
                    Line(format!(
                        "Time spent: {}",
                        Duration::realtime_elapsed(self.started)
                    )),
                ]));

                // Until the response is received, just ask winit to regularly call event(), so we
                // can keep polling the channel.
                ctx.request_update(UpdateType::Game);
                return Transition::Keep;
            }
            Ok(Some(Err(e))) => {
                error!("error in fetching data");
                let on_load = self.on_load.take().unwrap();
                return on_load(ctx, app, Err(e));
            }
            Ok(Some(Ok(builder))) => {
                debug!("future complete");
                let t = builder(app);
                let on_load = self.on_load.take().unwrap();
                return on_load(ctx, app, Ok(t));
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        g.clear(Color::BLACK);
        self.panel.draw(g);
    }
}

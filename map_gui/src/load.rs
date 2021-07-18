//! Loading large resources (like maps, scenarios, and prebaked data) requires different strategies
//! on native and web. Both cases are wrapped up as a State that runs a callback when done.

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use futures_channel::{mpsc, oneshot};
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
pub use native_loader::FileLoader;

#[cfg(target_arch = "wasm32")]
pub use wasm_loader::FileLoader;

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
                        map.map_loaded_directly();

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

// Use this with FileLoader to just read raw bytes without any deserialization.
struct RawBytes(Vec<u8>);

#[cfg(not(target_arch = "wasm32"))]
mod native_loader {
    use super::*;

    pub trait Readable {
        fn read_file(path: String, timer: &mut Timer) -> Result<Self>
        where
            Self: Sized;
    }

    /// Loads a JSON, bincoded, or raw file, then deserializes it
    pub struct FileLoader<A: AppLike, T> {
        path: String,
        // Wrapped in an Option just to make calling from event() work. Technically this is unsafe
        // if a caller fails to pop the FileLoader state in their transitions!
        on_load:
            Option<Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T>) -> Transition<A>>>,
    }

    impl<A: AppLike + 'static, T: 'static + Readable> FileLoader<A, T> {
        pub fn new_state(
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

    impl<A: AppLike + 'static, T: 'static + Readable> State<A> for FileLoader<A, T> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
            debug!("Loading {}", self.path);
            ctx.loading_screen(format!("load {}", self.path), |ctx, timer| {
                let file = T::read_file(self.path.clone(), timer);
                (self.on_load.take().unwrap())(ctx, app, timer, file)
            })
        }

        fn draw(&self, g: &mut GfxCtx, _: &A) {
            g.clear(Color::BLACK);
        }
    }

    // Two implementations for reading the file, using serde or just raw bytes
    impl<T: 'static + DeserializeOwned> Readable for T {
        fn read_file(path: String, timer: &mut Timer) -> Result<T> {
            abstio::read_object(path, timer)
        }
    }
    impl Readable for RawBytes {
        fn read_file(path: String, _: &mut Timer) -> Result<RawBytes> {
            abstio::slurp_file(path).map(RawBytes)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_loader {
    use std::io::Read;

    use futures::StreamExt;
    use wasm_bindgen::{JsCast, UnwrapThrowExt};
    use wasm_bindgen_futures::JsFuture;
    use wasm_streams::ReadableStream;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    use abstutil::prettyprint_usize;

    use super::*;

    pub trait Readable {
        fn read_url(url: String, resp: Vec<u8>) -> Result<Self>
        where
            Self: Sized;
    }

    /// Loads a JSON, bincoded, or raw file, then deserializes it
    ///
    /// Instead of blockingly reading a file within ctx.loading_screen, on the web have to
    /// asynchronously make an HTTP request and keep "polling" for completion in a way that's
    /// compatible with winit's event loop.
    pub struct FileLoader<A: AppLike, T> {
        response: oneshot::Receiver<Result<Vec<u8>>>,
        on_load:
            Option<Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T>) -> Transition<A>>>,
        panel: Panel,
        started: Instant,
        url: String,

        total_bytes: Option<usize>,
        read_bytes: usize,
        got_total_bytes: oneshot::Receiver<usize>,
        got_read_bytes: mpsc::Receiver<usize>,
    }

    impl<A: AppLike + 'static, T: 'static + Readable> FileLoader<A, T> {
        pub fn new_state(
            ctx: &mut EventCtx,
            path: String,
            on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, &mut Timer, Result<T>) -> Transition<A>>,
        ) -> Box<dyn State<A>> {
            let base_url = ctx
                .prerender
                .assets_base_url()
                .expect("assets_base_url must be specified for wasm builds via `Settings`");

            // Note that files are gzipped on S3 and other deployments. When running locally, we
            // just symlink the data/ directory, where files aren't compressed.
            let url = if ctx.prerender.assets_are_gzipped() {
                format!("{}/{}.gz", base_url, path)
            } else {
                format!("{}/{}", base_url, path)
            };

            // Make the HTTP request nonblockingly. When the response is received, send it through
            // the channel.
            let (tx, rx) = oneshot::channel();
            let (tx_total_bytes, got_total_bytes) = oneshot::channel();
            let (mut tx_read_bytes, got_read_bytes) = mpsc::channel(10);
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
                            let total_bytes = resp
                                .headers()
                                .get("Content-Length")
                                .unwrap()
                                .unwrap()
                                .parse::<usize>()
                                .unwrap();
                            tx_total_bytes.send(total_bytes).unwrap();

                            let raw_body = resp.body().unwrap_throw();
                            let body = ReadableStream::from_raw(raw_body.dyn_into().unwrap_throw());
                            let mut stream = body.into_stream();
                            let mut buffer = Vec::new();
                            while let Some(Ok(chunk)) = stream.next().await {
                                let array = js_sys::Uint8Array::new(&chunk);
                                if let Err(err) =
                                    tx_read_bytes.try_send(array.byte_length() as usize)
                                {
                                    warn!("Couldn't send update on bytes: {}", err);
                                }
                                // TODO Can we avoid this clone?
                                buffer.extend(array.to_vec());
                            }
                            tx.send(Ok(buffer)).unwrap();
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
                total_bytes: None,
                read_bytes: 0,
                got_total_bytes,
                got_read_bytes,
            })
        }
    }

    impl<A: AppLike + 'static, T: 'static + Readable> State<A> for FileLoader<A, T> {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
            if self.total_bytes.is_none() {
                if let Ok(Some(total)) = self.got_total_bytes.try_recv() {
                    self.total_bytes = Some(total);
                }
            }
            if let Some(read) = self.got_read_bytes.try_next().ok().and_then(|value| value) {
                self.read_bytes += read;
            }

            if let Some(maybe_resp) = self.response.try_recv().unwrap() {
                // TODO We stop drawing and start blocking at this point. It can take a
                // while. Any way to make it still be nonblockingish? Maybe put some of the work
                // inside that spawn_local?
                let mut timer = Timer::new(format!("Loading {}...", self.url));
                let result = maybe_resp.and_then(|resp| T::read_url(self.url.clone(), resp));
                return (self.on_load.take().unwrap())(ctx, app, &mut timer, result);
            }

            let mut lines = vec![
                Line(format!("Loading {}...", self.url)),
                Line(format!(
                    "Time spent: {}",
                    Duration::realtime_elapsed(self.started)
                )),
            ];
            if let Some(total) = self.total_bytes {
                lines.push(Line(format!(
                    "Read {} / {} bytes",
                    prettyprint_usize(self.read_bytes),
                    prettyprint_usize(total)
                )));
            }
            self.panel = ctx.make_loading_screen(Text::from_multiline(lines));

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

    // Two implementations for reading the file, using serde or just raw bytes
    impl<T: 'static + DeserializeOwned> Readable for T {
        fn read_url(url: String, resp: Vec<u8>) -> Result<T> {
            if url.ends_with(".gz") {
                let decoder = flate2::read::GzDecoder::new(&resp[..]);
                if url.ends_with(".bin.gz") {
                    abstutil::from_binary_reader(decoder)
                } else {
                    abstutil::from_json_reader(decoder)
                }
            } else if url.ends_with(".bin") {
                abstutil::from_binary(&&resp)
            } else {
                abstutil::from_json(&&resp)
            }
        }
    }
    impl Readable for RawBytes {
        fn read_url(url: String, resp: Vec<u8>) -> Result<RawBytes> {
            if url.ends_with(".gz") {
                let mut decoder = flate2::read::GzDecoder::new(&resp[..]);
                let mut buffer: Vec<u8> = Vec::new();
                decoder
                    .read_to_end(&mut buffer)
                    .map(|_| RawBytes(buffer))
                    .map_err(|err| err.into())
            } else {
                Ok(RawBytes(resp))
            }
        }
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
    // These're just two different types of progress updates that callers can provide
    outer_progress_receiver: Option<mpsc::Receiver<String>>,
    inner_progress_receiver: Option<mpsc::Receiver<String>>,
    last_outer_progress: String,
    last_inner_progress: String,

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
    pub fn new_state(
        ctx: &mut EventCtx,
        future: Pin<Box<dyn Future<Output = Result<Box<dyn Send + FnOnce(&A) -> T>>>>>,
        outer_progress_receiver: mpsc::Receiver<String>,
        inner_progress_receiver: mpsc::Receiver<String>,
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
            outer_progress_receiver: Some(outer_progress_receiver),
            inner_progress_receiver: Some(inner_progress_receiver),
            last_outer_progress: String::new(),
            last_inner_progress: String::new(),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_state(
        ctx: &mut EventCtx,
        future: Pin<Box<dyn Send + Future<Output = Result<Box<dyn Send + FnOnce(&A) -> T>>>>>,
        outer_progress_receiver: mpsc::Receiver<String>,
        inner_progress_receiver: mpsc::Receiver<String>,
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
            outer_progress_receiver: Some(outer_progress_receiver),
            inner_progress_receiver: Some(inner_progress_receiver),
            last_outer_progress: String::new(),
            last_inner_progress: String::new(),
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
                on_load(ctx, app, Err(anyhow!("channel canceled")))
            }
            Ok(None) => {
                if let Some(ref mut rx) = self.outer_progress_receiver {
                    // Read all of the progress that's happened
                    loop {
                        match rx.try_next() {
                            Ok(Some(msg)) => {
                                self.last_outer_progress = msg;
                            }
                            Ok(None) => {
                                self.outer_progress_receiver = None;
                                break;
                            }
                            Err(_) => {
                                // No messages
                                break;
                            }
                        }
                    }
                }
                if let Some(ref mut rx) = self.inner_progress_receiver {
                    loop {
                        match rx.try_next() {
                            Ok(Some(msg)) => {
                                self.last_inner_progress = msg;
                            }
                            Ok(None) => {
                                self.inner_progress_receiver = None;
                                break;
                            }
                            Err(_) => {
                                // No messages
                                break;
                            }
                        }
                    }
                }

                self.panel = ctx.make_loading_screen(Text::from_multiline(vec![
                    Line(&self.loading_title),
                    Line(format!(
                        "Time spent: {}",
                        Duration::realtime_elapsed(self.started)
                    )),
                    Line(&self.last_outer_progress),
                    Line(&self.last_inner_progress),
                ]));

                // Until the response is received, just ask winit to regularly call event(), so we
                // can keep polling the channel.
                ctx.request_update(UpdateType::Game);
                Transition::Keep
            }
            Ok(Some(Err(e))) => {
                error!("error in fetching data");
                let on_load = self.on_load.take().unwrap();
                on_load(ctx, app, Err(e))
            }
            Ok(Some(Ok(builder))) => {
                debug!("future complete");
                let t = builder(app);
                let on_load = self.on_load.take().unwrap();
                on_load(ctx, app, Ok(t))
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        g.clear(Color::BLACK);
        self.panel.draw(g);
    }
}

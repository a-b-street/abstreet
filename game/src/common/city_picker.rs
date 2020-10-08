use geom::{Distance, Polygon, Pt2D};
use map_model::City;
use widgetry::{
    Btn, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, ScreenPt, Text, Widget,
};

use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::nice_map_name;
use crate::render::DrawArea;

pub struct CityPicker {
    panel: Panel,
    // In untranslated screen-space
    regions: Vec<(String, Color, Polygon)>,
    selected: Option<usize>,
    on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
}

impl CityPicker {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State> {
        app.primary.current_selection = None;

        let mut batch = GeomBatch::new();
        let mut regions = Vec::new();

        if let Ok(city) = abstutil::maybe_read_binary::<City>(
            abstutil::path(format!(
                "system/cities/{}.bin",
                app.primary.map.get_city_name()
            )),
            &mut abstutil::Timer::throwaway(),
        ) {
            let bounds = city.boundary.get_bounds();

            let zoom = (0.8 * ctx.canvas.window_width / bounds.width())
                .min(0.8 * ctx.canvas.window_height / bounds.height());

            batch.push(app.cs.map_background.clone(), city.boundary);
            for (area_type, polygon) in city.areas {
                batch.push(DrawArea::fill(area_type, &app.cs), polygon);
            }

            for (name, polygon) in city.regions {
                let color = app.cs.rotating_color_agents(regions.len());
                if &name == app.primary.map.get_name() {
                    batch.push(color.alpha(0.5), polygon.clone());
                } else {
                    batch.push(color, polygon.to_outline(Distance::meters(200.0)).unwrap());
                }
                regions.push((name, color, polygon.scale(zoom)));
            }
            batch = batch.scale(zoom);
        }

        let mut other_cities = vec![Line("Other cities").draw(ctx)];
        let mut this_city = vec![];
        for name in abstutil::Manifest::load().all_map_names() {
            if let Some((_, color, _)) = regions.iter().find(|(n, _, _)| &name == n) {
                let btn = Btn::txt(&name, Text::from(Line(nice_map_name(&name)).fg(*color)))
                    .tooltip(Text::new());
                this_city.push(if &name == app.primary.map.get_name() {
                    btn.inactive(ctx)
                } else {
                    btn.build_def(ctx, None)
                });
            } else {
                other_cities.push(
                    Btn::txt(&name, Text::from(Line(nice_map_name(&name))))
                        .tooltip(Text::new())
                        .build_def(ctx, None),
                );
            }
        }

        Box::new(CityPicker {
            regions,
            selected: None,
            on_load,
            panel: Panel::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Select a region").small_heading().draw(ctx),
                        Btn::plaintext("X")
                            .build(ctx, "close", Key::Escape)
                            .align_right(),
                    ]),
                    Widget::row(vec![
                        Widget::col(other_cities).centered_vert(),
                        Widget::draw_batch(ctx, batch).named("picker"),
                        Widget::col(this_city).centered_vert(),
                    ]),
                ])
                .outline(2.0, Color::WHITE),
            )
            .build(ctx),
        })
    }
}

impl State for CityPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                name => {
                    let on_load =
                        std::mem::replace(&mut self.on_load, Box::new(|_, _| Transition::Keep));
                    return switch_map(ctx, app, name, on_load);
                }
            },
            _ => {}
        }

        if ctx.redo_mouseover() {
            self.selected = None;
            if let Some(cursor) = ctx.canvas.get_cursor_in_screen_space() {
                let rect = self.panel.rect_of("picker");
                if rect.contains(cursor) {
                    let pt = Pt2D::new(cursor.x - rect.x1, cursor.y - rect.y1);
                    for (idx, (name, _, poly)) in self.regions.iter().enumerate() {
                        if name != app.primary.map.get_name() && poly.contains_pt(pt) {
                            self.selected = Some(idx);
                            break;
                        }
                    }
                } else if let Some(btn) = self.panel.currently_hovering() {
                    for (idx, (name, _, _)) in self.regions.iter().enumerate() {
                        if name != app.primary.map.get_name() && name == btn {
                            self.selected = Some(idx);
                            break;
                        }
                    }
                }
            }
        }
        if let Some(idx) = self.selected {
            let name = &self.regions[idx].0;
            if app
                .per_obj
                .left_click(ctx, format!("switch to {}", nice_map_name(name)))
            {
                return ctx.loading_screen("switch map", |ctx, _| {
                    app.switch_map(ctx, abstutil::path_map(name));
                    (self.on_load)(ctx, app)
                });
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.panel.draw(g);

        if let Some(idx) = self.selected {
            let (name, color, poly) = &self.regions[idx];
            let rect = self.panel.rect_of("picker");
            g.fork(
                Pt2D::new(0.0, 0.0),
                ScreenPt::new(rect.x1, rect.y1),
                1.0,
                None,
            );
            g.draw_polygon(color.alpha(0.5), poly.clone());
            g.unfork();

            g.draw_mouse_tooltip(Text::from(Line(nice_map_name(name))));
        }
    }
}

// Natively, we can blockingly load from the filesystem as usual.
#[cfg(not(target_arch = "wasm32"))]
fn switch_map(
    ctx: &mut EventCtx,
    app: &mut App,
    name: &str,
    on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
) -> Transition {
    if abstutil::file_exists(abstutil::path_map(&name)) {
        ctx.loading_screen("switch map", |ctx, _| {
            app.switch_map(ctx, abstutil::path_map(name));
            (on_load)(ctx, app)
        })
    } else {
        // TODO Some kind of UI for running the updater from here!
        Transition::Replace(crate::game::PopupMsg::new(
            ctx,
            "Missing data",
            vec![
                format!("{} is missing", abstutil::path_map(&name)),
                "You need to opt into this by modifying data/config and running the updater"
                    .to_string(),
            ],
        ))
    }
}

// On the web, we asynchronously download the map file.
#[cfg(target_arch = "wasm32")]
fn switch_map(
    ctx: &mut EventCtx,
    _: &mut App,
    name: &str,
    on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
) -> Transition {
    Transition::Replace(loader::AsyncFileLoader::new(
        ctx,
        format!("http://0.0.0.0:8000/system/maps/{}.bin", name),
        on_load,
    ))
}

#[cfg(target_arch = "wasm32")]
mod loader {
    use futures_channel::oneshot;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    use abstutil::Timer;
    use map_model::Map;
    use widgetry::UpdateType;

    use super::*;
    use crate::render::DrawMap;

    // Instead of blockingly reading a file within ctx.loading_screen, on the web have to
    // asynchronously make an HTTP request and keep "polling" for completion in a way that's
    // compatible with winit's event loop.
    pub struct AsyncFileLoader {
        response: oneshot::Receiver<Vec<u8>>,
        on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
        panel: Panel,
    }

    impl AsyncFileLoader {
        pub fn new(
            ctx: &mut EventCtx,
            url: String,
            on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
        ) -> Box<dyn State> {
            let panel = ctx.make_loading_screen(Text::from(Line(format!("Loading {}...", url))));

            // Make the HTTP request nonblockingly. When the response is received, send it through
            // the channel.
            let (tx, rx) = oneshot::channel();
            wasm_bindgen_futures::spawn_local(async move {
                let mut opts = RequestInit::new();
                opts.method("GET");
                opts.mode(RequestMode::Cors);
                let request = Request::new_with_str_and_init(&url, &opts).unwrap();

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

            Box::new(AsyncFileLoader {
                response: rx,
                on_load,
                panel,
            })
        }
    }

    impl State for AsyncFileLoader {
        fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
            if let Some(resp) = self.response.try_recv().unwrap() {
                let map: Map = abstutil::from_binary(&resp).unwrap();

                // TODO This is a hack, repeating only some parts of app.switch_map. Refactor.
                let bounds = map.get_bounds();
                ctx.canvas.map_dims = (bounds.width(), bounds.height());
                app.primary.map = map;
                let mut timer = Timer::new("switch maps");
                app.primary.draw_map =
                    DrawMap::new(&app.primary.map, &app.opts, &app.cs, ctx, &mut timer);
                app.primary.clear_sim();

                return (self.on_load)(ctx, app);
            }

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

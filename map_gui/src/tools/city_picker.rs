use abstio::MapName;
use abstutil::Timer;
use geom::{Distance, Percent, Polygon, Pt2D};
use map_model::City;
use widgetry::{
    Autocomplete, Btn, Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome,
    Panel, ScreenPt, State, Text, TextExt, Transition, Widget,
};

use crate::load::MapLoader;
use crate::render::DrawArea;
use crate::tools::{grey_out_map, nice_map_name, open_browser};
use crate::AppLike;

/// Lets the player switch maps.
pub struct CityPicker<A: AppLike> {
    panel: Panel,
    // In untranslated screen-space
    regions: Vec<(MapName, Color, Polygon)>,
    selected: Option<usize>,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> CityPicker<A> {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut A,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let city = app.map().get_city_name().clone();
        CityPicker::new_in_city(ctx, app, on_load, city)
    }

    fn new_in_city(
        ctx: &mut EventCtx,
        app: &mut A,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
        city_name: String,
    ) -> Box<dyn State<A>> {
        let mut batch = GeomBatch::new();
        let mut regions = Vec::new();
        let mut this_city = vec![];

        // If this city overview doesn't exist, we assume this map is the only one in the city.
        if let Ok(city) = abstio::maybe_read_binary::<City>(
            abstio::path(format!("system/{}/city.bin", city_name)),
            &mut Timer::throwaway(),
        ) {
            let bounds = city.boundary.get_bounds();

            let zoom = (0.8 * ctx.canvas.window_width / bounds.width())
                .min(0.8 * ctx.canvas.window_height / bounds.height());

            batch.push(app.cs().map_background.clone(), city.boundary);
            for (area_type, polygon) in city.areas {
                batch.push(DrawArea::fill(area_type, app.cs()), polygon);
            }

            for (name, polygon) in city.regions {
                let color = app.cs().rotating_color_agents(regions.len());

                let btn = Btn::txt(
                    name.path(),
                    Text::from(Line(nice_map_name(&name)).fg(color)),
                )
                .no_tooltip();

                if &name == app.map().get_name() {
                    this_city.push(btn.inactive(ctx));
                    batch.push(color.alpha(0.5), polygon.clone());
                } else {
                    this_city.push(btn.build_def(ctx, None));
                    batch.push(color, polygon.to_outline(Distance::meters(200.0)).unwrap());
                    regions.push((name, color, polygon.scale(zoom)));
                }
            }
            batch = batch.scale(zoom);

            this_city.insert(0, format!("More regions in {}", city_name).draw_text(ctx));
        }

        let mut other_cities = vec![Line("Other cities").draw(ctx)];
        for city in MapName::list_all_cities() {
            if city == city_name {
                continue;
            }
            // If there's only one map in the city, make the button directly load it.
            let maps = MapName::list_all_maps_in_city(&city);
            if maps.len() == 1 {
                other_cities.push(Btn::text_fg(city).build(ctx, maps[0].path(), None));
            } else {
                other_cities.push(Btn::text_fg(city).no_tooltip().build_def(ctx, None));
            }
        }
        other_cities.push(Btn::text_bg2("Search all maps").build_def(ctx, Key::Tab));

        Box::new(CityPicker {
            regions,
            selected: None,
            on_load: Some(on_load),
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a region").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Widget::row(vec![
                    Widget::col(other_cities).centered_vert(),
                    Widget::draw_batch(ctx, batch).named("picker"),
                    Widget::col(this_city).centered_vert(),
                ]),
                Widget::custom_row(vec![
                    "Don't see the city you want?"
                        .draw_text(ctx)
                        .centered_vert(),
                    Btn::plaintext_custom(
                        "import new city",
                        Text::from(
                            Line("Import a new city into A/B Street")
                                .fg(Color::hex("#4CA4E5"))
                                .underlined(),
                        ),
                    )
                    .build_def(ctx, None),
                ]),
                if cfg!(not(target_arch = "wasm32")) {
                    Btn::text_fg("Download more cities").build_def(ctx, None)
                } else {
                    Widget::nothing()
                },
            ]))
            .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for CityPicker<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Search all maps" => {
                    return Transition::Replace(AllCityPicker::new(
                        ctx,
                        self.on_load.take().unwrap(),
                    ));
                }
                "import new city" => {
                    open_browser(
                        "https://dabreegster.github.io/abstreet/howto/new_city.html".to_string(),
                    );
                }
                "Download more cities" => {
                    let _ = "just stop this from counting as an attribute on an expression";
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        return Transition::Replace(crate::tools::updater::Picker::new(
                            ctx,
                            self.on_load.take().unwrap(),
                        ));
                    }
                }
                x => {
                    if let Some(name) = MapName::from_path(x) {
                        return Transition::Replace(MapLoader::new(
                            ctx,
                            app,
                            name,
                            self.on_load.take().unwrap(),
                        ));
                    }
                    // Browse maps for another city without loading any map there
                    return Transition::Replace(CityPicker::new_in_city(
                        ctx,
                        app,
                        self.on_load.take().unwrap(),
                        x.to_string(),
                    ));
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
                    for (idx, (_, _, poly)) in self.regions.iter().enumerate() {
                        if poly.contains_pt(pt) {
                            self.selected = Some(idx);
                            break;
                        }
                    }
                } else if let Some(btn) = self.panel.currently_hovering() {
                    for (idx, (name, _, _)) in self.regions.iter().enumerate() {
                        if &name.map == btn {
                            self.selected = Some(idx);
                            break;
                        }
                    }
                }
            }
        }
        if let Some(idx) = self.selected {
            let name = &self.regions[idx].0;
            if ctx.normal_left_click() {
                return Transition::Replace(MapLoader::new(
                    ctx,
                    app,
                    name.clone(),
                    self.on_load.take().unwrap(),
                ));
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
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

struct AllCityPicker<A: AppLike> {
    panel: Panel,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> AllCityPicker<A> {
    fn new(
        ctx: &mut EventCtx,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let mut autocomplete_entries = Vec::new();
        let mut buttons = Vec::new();

        for name in MapName::list_all_maps() {
            buttons.push(
                Btn::text_fg(name.describe())
                    .build(ctx, name.path(), None)
                    .margin_right(10)
                    .margin_below(10),
            );
            autocomplete_entries.push((name.describe(), name.path()));
        }

        Box::new(AllCityPicker {
            on_load: Some(on_load),
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a region").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Widget::row(vec![
                    Widget::draw_svg(ctx, "system/assets/tools/search.svg"),
                    Autocomplete::new(ctx, autocomplete_entries).named("search"),
                ])
                .padding(8),
                Widget::custom_row(buttons).flex_wrap(ctx, Percent::int(70)),
            ]))
            .exact_size_percent(80, 80)
            .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for AllCityPicker<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                path => {
                    return Transition::Replace(MapLoader::new(
                        ctx,
                        app,
                        MapName::from_path(path).unwrap(),
                        self.on_load.take().unwrap(),
                    ));
                }
            },
            _ => {}
        }
        if let Some(mut paths) = self.panel.autocomplete_done::<String>("search") {
            if !paths.is_empty() {
                return Transition::Replace(MapLoader::new(
                    ctx,
                    app,
                    MapName::from_path(&paths.remove(0)).unwrap(),
                    self.on_load.take().unwrap(),
                ));
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

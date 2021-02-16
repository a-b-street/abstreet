use std::collections::BTreeMap;

use abstio::{CityName, MapName};
use geom::{Distance, Percent, Polygon, Pt2D};
use map_model::City;
use widgetry::{
    Autocomplete, Color, ControlState, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line,
    Outcome, Panel, RewriteColor, ScreenPt, State, StyledButtons, Text, TextExt, Transition,
    Widget,
};

use crate::load::{FileLoader, MapLoader};
use crate::render::DrawArea;
use crate::tools::{grey_out_map, nice_country_name, nice_map_name, open_browser};
use crate::AppLike;

/// Lets the player switch maps.
pub struct CityPicker<A: AppLike> {
    panel: Panel,
    // In untranslated screen-space
    districts: Vec<(MapName, Color, Polygon)>,
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
        CityPicker::new_in_city(ctx, on_load, city)
    }

    fn new_in_city(
        ctx: &mut EventCtx,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
        city_name: CityName,
    ) -> Box<dyn State<A>> {
        FileLoader::<A, City>::new(
            ctx,
            abstio::path(format!(
                "system/{}/{}/city.bin",
                city_name.country, city_name.city
            )),
            Box::new(move |ctx, app, _, maybe_city| {
                let mut batch = GeomBatch::new();
                let mut districts = Vec::new();
                let mut this_city = vec![];

                // If this city overview doesn't exist, we assume this map is the only one in the
                // city.
                if let Ok(city) = maybe_city {
                    let bounds = city.boundary.get_bounds();

                    let zoom = (0.8 * ctx.canvas.window_width / bounds.width())
                        .min(0.8 * ctx.canvas.window_height / bounds.height());

                    batch.push(app.cs().map_background.clone(), city.boundary);
                    for (area_type, polygon) in city.areas {
                        batch.push(DrawArea::fill(area_type, app.cs()), polygon);
                    }

                    let mut buttons = Vec::new();
                    for (name, polygon) in city.regions {
                        let color = app.cs().rotating_color_agents(districts.len());

                        let btn = ctx
                            .style()
                            .btn_outline_light_text(nice_map_name(&name))
                            .no_tooltip();

                        let action = name.path();
                        if &name == app.map().get_name() {
                            let btn = btn.disabled(true);
                            buttons.push((name.clone(), btn.build_widget(ctx, &action)));
                        } else {
                            buttons.push((name.clone(), btn.build_widget(ctx, &action)));
                            batch.push(color, polygon.to_outline(Distance::meters(200.0)).unwrap());
                            districts.push((name, color, polygon.scale(zoom)));
                        }
                    }
                    batch = batch.scale(zoom);

                    this_city
                        .push(format!("More districts in {}", city_name.describe()).draw_text(ctx));
                    // city.regions are sorted in an order necessary for z-ordering (larger regions
                    // last), but we want the buttons listed on the side to be alphabetical.
                    buttons.sort_by_key(|(name, _)| name.clone());
                    for (_, btn) in buttons {
                        this_city.push(btn);
                    }
                }

                let mut other_places = vec![Line("Other places").draw(ctx)];
                for (country, cities) in cities_per_country() {
                    // If there's only one city and we're already there, skip it.
                    if cities.len() == 1 && cities[0] == city_name {
                        continue;
                    }
                    other_places.push(
                        ctx.style()
                            .btn_outline_light_icon_text(
                                &format!("system/assets/flags/{}.svg", country),
                                &format!("{} in {}", cities.len(), nice_country_name(&country)),
                            )
                            .image_color(RewriteColor::NoOp, ControlState::Default)
                            .image_dims(30.0)
                            .build_widget(ctx, &country),
                    );
                }
                other_places.push(
                    ctx.style()
                        .btn_solid_dark_text("Search all maps")
                        .hotkey(Key::Tab)
                        .build_def(ctx),
                );

                Transition::Replace(Box::new(CityPicker {
                    districts,
                    selected: None,
                    on_load: Some(on_load),
                    panel: Panel::new(Widget::col(vec![
                        Widget::row(vec![
                            Line("Select a district").small_heading().draw(ctx),
                            ctx.style().btn_close_widget(ctx),
                        ]),
                        Widget::row(vec![
                            Widget::col(other_places).centered_vert(),
                            Widget::draw_batch(ctx, batch).named("picker"),
                            Widget::col(this_city).centered_vert(),
                        ]),
                        Widget::custom_row(vec![
                            "Don't see the city you want?"
                                .draw_text(ctx)
                                .centered_vert(),
                            ctx.style()
                                .btn_plain_light()
                                .label_styled_text(
                                    Text::from(
                                        Line("Import a new city into A/B Street")
                                            .fg(Color::hex("#4CA4E5"))
                                            .underlined(),
                                    ),
                                    ControlState::Default,
                                )
                                .build_widget(ctx, "import new city"),
                        ]),
                        if cfg!(not(target_arch = "wasm32")) {
                            ctx.style()
                                .btn_outline_light_text("Download more cities")
                                .build_def(ctx)
                        } else {
                            Widget::nothing()
                        },
                    ]))
                    .build(ctx),
                }))
            }),
        )
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
                    open_browser("https://a-b-street.github.io/docs/howto/new_city.html");
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
                    // Browse cities for another country
                    return Transition::Replace(CitiesInCountryPicker::new(
                        ctx,
                        app,
                        self.on_load.take().unwrap(),
                        x,
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
                    for (idx, (_, _, poly)) in self.districts.iter().enumerate() {
                        if poly.contains_pt(pt) {
                            self.selected = Some(idx);
                            break;
                        }
                    }
                } else if let Some(btn) = self.panel.currently_hovering() {
                    for (idx, (name, _, _)) in self.districts.iter().enumerate() {
                        if &name.path() == btn {
                            self.selected = Some(idx);
                            break;
                        }
                    }
                }
            }
        }
        if let Some(idx) = self.selected {
            let name = &self.districts[idx].0;
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
            let (name, color, poly) = &self.districts[idx];
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
                ctx.style()
                    .btn_outline_light_text(&name.describe())
                    .build_widget(ctx, &name.path())
                    .margin_right(10)
                    .margin_below(10),
            );
            autocomplete_entries.push((name.describe(), name.path()));
        }

        Box::new(AllCityPicker {
            on_load: Some(on_load),
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a district").small_heading().draw(ctx),
                    ctx.style().btn_close_widget(ctx),
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

struct CitiesInCountryPicker<A: AppLike> {
    panel: Panel,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> CitiesInCountryPicker<A> {
    fn new(
        ctx: &mut EventCtx,
        app: &A,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
        country: &str,
    ) -> Box<dyn State<A>> {
        let mut buttons = Vec::new();
        for city in cities_per_country().remove(country).unwrap() {
            if &city == app.map().get_city_name() {
                continue;
            }
            buttons.push(
                ctx.style()
                    .btn_outline_light_text(&city.city)
                    .build_widget(ctx, &city.to_path())
                    .margin_right(10)
                    .margin_below(10),
            );
        }

        let flag = GeomBatch::load_svg(ctx, &format!("system/assets/flags/{}.svg", country));
        let y_factor = 30.0 / flag.get_dims().height;

        Box::new(CitiesInCountryPicker {
            on_load: Some(on_load),
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_batch(ctx, flag.scale(y_factor)),
                    Line(format!("Select a city in {}", nice_country_name(country)))
                        .small_heading()
                        .draw(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::custom_row(buttons).flex_wrap(ctx, Percent::int(70)),
            ]))
            .exact_size_percent(80, 80)
            .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for CitiesInCountryPicker<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    // Go back to the screen that lets you choose all countries.
                    return Transition::Replace(CityPicker::new(
                        ctx,
                        app,
                        self.on_load.take().unwrap(),
                    ));
                }
                path => {
                    let city = CityName::parse(path).unwrap();
                    let mut maps = MapName::list_all_maps_in_city(&city);
                    if maps.len() == 1 {
                        return Transition::Replace(MapLoader::new(
                            ctx,
                            app,
                            maps.pop().unwrap(),
                            self.on_load.take().unwrap(),
                        ));
                    }
                    return Transition::Replace(CityPicker::new_in_city(
                        ctx,
                        self.on_load.take().unwrap(),
                        city,
                    ));
                }
            },
            _ => {}
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

fn cities_per_country() -> BTreeMap<String, Vec<CityName>> {
    let mut per_country = BTreeMap::new();
    for city in CityName::list_all_cities_from_system_data() {
        per_country
            .entry(city.country.clone())
            .or_insert_with(Vec::new)
            .push(city);
    }
    per_country
}

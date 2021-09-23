use std::collections::BTreeMap;

use abstio::{CityName, Manifest, MapName};
use geom::{Distance, Percent};
use map_model::City;
use widgetry::{
    lctrl, Autocomplete, ClickOutcome, ControlState, DrawBaselayer, DrawWithTooltips, EventCtx,
    GeomBatch, GfxCtx, Image, Key, Line, Outcome, Panel, RewriteColor, State, Text, TextExt,
    Transition, Widget,
};

use crate::load::{FileLoader, MapLoader};
use crate::render::DrawArea;
use crate::tools::{grey_out_map, nice_country_name, nice_map_name};
use crate::AppLike;

/// Lets the player switch maps.
pub struct CityPicker<A: AppLike> {
    panel: Panel,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> CityPicker<A> {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &A,
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
        FileLoader::<A, City>::new_state(
            ctx,
            abstio::path(format!(
                "system/{}/{}/city.bin",
                city_name.country, city_name.city
            )),
            Box::new(move |ctx, app, _, maybe_city| {
                // If city.bin exists, use it to draw the district map.
                let district_picker = if let Ok(city) = maybe_city {
                    let bounds = city.boundary.get_bounds();

                    let zoom = (0.8 * ctx.canvas.window_width / bounds.width())
                        .min(0.8 * ctx.canvas.window_height / bounds.height());

                    let mut batch = GeomBatch::new();
                    batch.push(app.cs().map_background.clone(), city.boundary);
                    for (area_type, polygon) in city.areas {
                        batch.push(DrawArea::fill(area_type, app.cs()), polygon);
                    }

                    // If somebody has just generated a new map somewhere with an existing
                    // city.bin, but hasn't updated city.bin yet, that new map will be invisible on
                    // the city-wide diagram.
                    let outline_color = app.cs().minimap_cursor_border;
                    let mut tooltips = Vec::new();
                    for (name, polygon) in city.districts {
                        if &name != app.map().get_name() {
                            batch.push(
                                outline_color,
                                polygon.to_outline(Distance::meters(200.0)).unwrap(),
                            );
                            let polygon = polygon.scale(zoom);
                            tooltips.push((
                                polygon.clone(),
                                Text::from(nice_map_name(&name)),
                                Some(ClickOutcome::Custom(Box::new(name))),
                            ));
                        }
                    }
                    DrawWithTooltips::new_widget(
                        ctx,
                        batch.scale(zoom),
                        tooltips,
                        Box::new(move |poly| {
                            GeomBatch::from(vec![(outline_color.alpha(0.5), poly.clone())])
                        }),
                    )
                } else {
                    Widget::nothing()
                };

                // Use the filesystem to list the buttons on the side.
                // (There's no point in listing these from city.bin if it exists -- if somebody
                // imports a new map in an existing city, it could be out of sync anyway.)
                let mut this_city =
                    vec![format!("More districts in {}", city_name.describe()).text_widget(ctx)];
                for name in MapName::list_all_maps_in_city_merged(&city_name, &Manifest::load()) {
                    this_city.push(
                        ctx.style()
                            .btn_outline
                            .text(nice_map_name(&name))
                            .no_tooltip()
                            .disabled(&name == app.map().get_name())
                            .build_widget(ctx, &name.path()),
                    );
                }

                let mut other_places = vec![Line("Other places").into_widget(ctx)];
                for (country, cities) in cities_per_country() {
                    // If there's only one city and we're already there, skip it.
                    if cities.len() == 1 && cities[0] == city_name {
                        continue;
                    }
                    let flag_path = format!("system/assets/flags/{}.svg", country);
                    if abstio::file_exists(abstio::path(&flag_path)) {
                        other_places.push(
                            ctx.style()
                                .btn_outline
                                .icon_text(
                                    &flag_path,
                                    format!("{} in {}", cities.len(), nice_country_name(&country)),
                                )
                                .image_color(RewriteColor::NoOp, ControlState::Default)
                                .image_dims(30.0)
                                .build_widget(ctx, &country),
                        );
                    } else {
                        other_places.push(
                            ctx.style()
                                .btn_outline
                                .text(format!(
                                    "{} in {}",
                                    cities.len(),
                                    nice_country_name(&country)
                                ))
                                .build_widget(ctx, country),
                        );
                    }
                }

                Transition::Replace(Box::new(CityPicker {
                    on_load: Some(on_load),
                    panel: Panel::new_builder(Widget::col(vec![
                        Widget::row(vec![
                            Line("Select a district").small_heading().into_widget(ctx),
                            ctx.style().btn_close_widget(ctx),
                        ]),
                        if cfg!(target_arch = "wasm32") {
                            // On web, this is a link, so it's styled appropriately.
                            ctx.style()
                                .btn_plain
                                .btn()
                                .label_underlined_text("Import a new city into A/B Street")
                                .build_widget(ctx, "import new city")
                        } else {
                            // On native this shows the "import" instructions modal within
                            // the app
                            ctx.style()
                                .btn_outline
                                .text("Import a new city into A/B Street")
                                .build_widget(ctx, "import new city")
                        },
                        ctx.style()
                            .btn_outline
                            .icon_text("system/assets/tools/search.svg", "Search all maps")
                            .hotkey(lctrl(Key::F))
                            .build_def(ctx),
                        Widget::row(vec![
                            Widget::col(other_places).centered_vert(),
                            district_picker,
                            Widget::col(this_city).centered_vert(),
                        ]),
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
                    return Transition::Replace(AllCityPicker::new_state(
                        ctx,
                        self.on_load.take().unwrap(),
                    ));
                }
                "import new city" => {
                    #[cfg(target_arch = "wasm32")]
                    {
                        crate::tools::open_browser(
                            "https://a-b-street.github.io/docs/user/new_city.html",
                        );
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        return Transition::Replace(crate::tools::importer::ImportCity::new_state(
                            ctx,
                            self.on_load.take().unwrap(),
                        ));
                    }
                }
                x => {
                    if let Some(name) = MapName::from_path(x) {
                        return chose_city(ctx, app, name, &mut self.on_load);
                    }
                    // Browse cities for another country
                    return Transition::Replace(CitiesInCountryPicker::new_state(
                        ctx,
                        app,
                        self.on_load.take().unwrap(),
                        x,
                    ));
                }
            },
            Outcome::ClickCustom(data) => {
                let name = data.as_any().downcast_ref::<MapName>().unwrap();
                return chose_city(ctx, app, name.clone(), &mut self.on_load);
            }
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

struct AllCityPicker<A: AppLike> {
    panel: Panel,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> AllCityPicker<A> {
    fn new_state(
        ctx: &mut EventCtx,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
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

        Box::new(AllCityPicker {
            on_load: Some(on_load),
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a district").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    Image::from_path("system/assets/tools/search.svg").into_widget(ctx),
                    Autocomplete::new_widget(ctx, autocomplete_entries, 10).named("search"),
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
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                path => {
                    return chose_city(
                        ctx,
                        app,
                        MapName::from_path(path).unwrap(),
                        &mut self.on_load,
                    );
                }
            }
        }
        if let Some(mut paths) = self.panel.autocomplete_done::<String>("search") {
            if !paths.is_empty() {
                return chose_city(
                    ctx,
                    app,
                    MapName::from_path(&paths.remove(0)).unwrap(),
                    &mut self.on_load,
                );
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
    fn new_state(
        ctx: &mut EventCtx,
        app: &A,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
        country: &str,
    ) -> Box<dyn State<A>> {
        let flag_path = format!("system/assets/flags/{}.svg", country);
        let draw_flag = if abstio::file_exists(abstio::path(&flag_path)) {
            let flag = GeomBatch::load_svg(ctx, format!("system/assets/flags/{}.svg", country));
            let y_factor = 30.0 / flag.get_dims().height;
            flag.scale(y_factor).into_widget(ctx)
        } else {
            Widget::nothing()
        };
        let mut col = vec![Widget::row(vec![
            draw_flag,
            Line(format!("Select a city in {}", nice_country_name(country)))
                .small_heading()
                .into_widget(ctx),
            ctx.style().btn_close_widget(ctx),
        ])];

        let mut buttons = Vec::new();
        let mut last_letter = ' ';
        for city in cities_per_country().remove(country).unwrap() {
            if &city == app.map().get_city_name() {
                continue;
            }
            let letter = city
                .city
                .chars()
                .next()
                .unwrap()
                .to_uppercase()
                .next()
                .unwrap();
            if last_letter != letter {
                if !buttons.is_empty() {
                    let mut row = vec![Line(last_letter)
                        .small_heading()
                        .into_widget(ctx)
                        .margin_right(20)];
                    row.extend(buttons.drain(..));
                    col.push(
                        Widget::custom_row(row).flex_wrap_no_inner_spacing(ctx, Percent::int(70)),
                    );
                }

                last_letter = letter;
            }

            buttons.push(
                ctx.style()
                    .btn_outline
                    .text(&city.city)
                    .build_widget(ctx, &city.to_path())
                    .margin_right(10)
                    .margin_below(10),
            );
        }
        if !buttons.is_empty() {
            let mut row = vec![Line(last_letter)
                .small_heading()
                .into_widget(ctx)
                .margin_right(20)];
            row.extend(buttons.drain(..));
            col.push(Widget::custom_row(row).flex_wrap_no_inner_spacing(ctx, Percent::int(70)));
        }

        Box::new(CitiesInCountryPicker {
            on_load: Some(on_load),
            panel: Panel::new_builder(Widget::col(col))
                .exact_size_percent(80, 80)
                .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for CitiesInCountryPicker<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    // Go back to the screen that lets you choose all countries.
                    return Transition::Replace(CityPicker::new_state(
                        ctx,
                        app,
                        self.on_load.take().unwrap(),
                    ));
                }
                path => {
                    let city = CityName::parse(path).unwrap();
                    let mut maps = MapName::list_all_maps_in_city_merged(&city, &Manifest::load());
                    if maps.len() == 1 {
                        return chose_city(ctx, app, maps.pop().unwrap(), &mut self.on_load);
                    }

                    // We may need to grab city.bin
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let path = format!("system/{}/{}/city.bin", city.country, city.city);
                        if Manifest::load()
                            .entries
                            .contains_key(&format!("data/{}", path))
                            && !abstio::file_exists(abstio::path(path))
                        {
                            return crate::tools::prompt_to_download_missing_data(
                                ctx,
                                maps.pop().unwrap(),
                            );
                        }
                    }

                    return Transition::Replace(CityPicker::new_in_city(
                        ctx,
                        self.on_load.take().unwrap(),
                        city,
                    ));
                }
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

fn cities_per_country() -> BTreeMap<String, Vec<CityName>> {
    let mut per_country = BTreeMap::new();
    for city in CityName::list_all_cities_merged(&Manifest::load()) {
        per_country
            .entry(city.country.clone())
            .or_insert_with(Vec::new)
            .push(city);
    }
    per_country
}

fn chose_city<A: AppLike + 'static>(
    ctx: &mut EventCtx,
    app: &mut A,
    name: MapName,
    on_load: &mut Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
) -> Transition<A> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !abstio::file_exists(name.path()) {
            return crate::tools::prompt_to_download_missing_data(ctx, name);
        }
    }

    Transition::Replace(MapLoader::new_state(
        ctx,
        app,
        name,
        on_load.take().unwrap(),
    ))
}

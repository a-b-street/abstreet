use std::collections::BTreeMap;

use abstio::{CityName, Manifest, MapName};
use geom::{Distance, Percent, Polygon, Pt2D};
use map_model::City;
use widgetry::{
    Autocomplete, Color, ControlState, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Image, Key,
    Line, Outcome, Panel, RewriteColor, ScreenPt, State, Text, TextExt, Transition, Widget,
};

use crate::load::{FileLoader, MapLoader};
use crate::render::DrawArea;
use crate::tools::{grey_out_map, nice_country_name, nice_map_name};
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
    pub fn new_state(
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
        FileLoader::<A, City>::new_state(
            ctx,
            abstio::path(format!(
                "system/{}/{}/city.bin",
                city_name.country, city_name.city
            )),
            Box::new(move |ctx, app, _, maybe_city| {
                let mut batch = GeomBatch::new();
                let mut districts = Vec::new();
                let mut this_city =
                    vec![format!("More districts in {}", city_name.describe()).text_widget(ctx)];

                if let Ok(city) = maybe_city {
                    let bounds = city.boundary.get_bounds();

                    let zoom = (0.8 * ctx.canvas.window_width / bounds.width())
                        .min(0.8 * ctx.canvas.window_height / bounds.height());

                    batch.push(app.cs().map_background.clone(), city.boundary);
                    for (area_type, polygon) in city.areas {
                        batch.push(DrawArea::fill(area_type, app.cs()), polygon);
                    }

                    // If somebody has just generated a new map somewhere with an existing
                    // city.bin, but hasn't updated city.bin yet, that new map will be invisible.
                    let mut buttons = Vec::new();
                    for (name, polygon) in city.districts {
                        let color = app.cs().rotating_color_agents(districts.len());

                        let btn = ctx
                            .style()
                            .btn_outline
                            .text(nice_map_name(&name))
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

                    // city.districts are sorted in an order necessary for z-ordering (larger
                    // districts last), but we want the buttons listed on the side to be
                    // alphabetical.
                    buttons.sort_by_key(|(name, _)| name.clone());
                    for (_, btn) in buttons {
                        this_city.push(btn);
                    }
                } else {
                    // If this city overview doesn't exist, just list files.
                    // If we have the city.bin, then we ought to have all the files locally.
                    for name in MapName::list_all_maps_in_city_locally(&city_name) {
                        this_city.push(
                            ctx.style()
                                .btn_outline
                                .text(nice_map_name(&name))
                                .no_tooltip()
                                .disabled(&name == app.map().get_name())
                                .build_widget(ctx, &name.path()),
                        );
                    }
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
                other_places.push(
                    ctx.style()
                        .btn_outline
                        .text("Search all maps")
                        .hotkey(Key::Tab)
                        .build_def(ctx),
                );

                Transition::Replace(Box::new(CityPicker {
                    districts,
                    selected: None,
                    on_load: Some(on_load),
                    panel: Panel::new(Widget::col(vec![
                        Widget::row(vec![
                            Line("Select a district").small_heading().into_widget(ctx),
                            ctx.style().btn_close_widget(ctx),
                        ]),
                        Widget::row(vec![
                            Widget::col(other_places).centered_vert(),
                            batch.into_widget(ctx).named("picker"),
                            Widget::col(this_city).centered_vert(),
                        ]),
                        "Don't see the place you want?".text_widget(ctx),
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
                    ]))
                    .build(ctx),
                }))
            }),
        )
    }
}

impl<A: AppLike + 'static> State<A> for CityPicker<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
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
                            "https://a-b-street.github.io/docs/howto/new_city.html",
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
            }
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
            if ctx.normal_left_click() {
                return chose_city(ctx, app, self.districts[idx].0.clone(), &mut self.on_load);
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

            g.draw_mouse_tooltip(Text::from(nice_map_name(name)));
        }
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

        for name in MapName::list_all_maps_from_manifest(&Manifest::load()) {
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
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a district").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    Image::from_path("system/assets/tools/search.svg").into_widget(ctx),
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
            panel: Panel::new(Widget::col(col))
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
                    let mut maps =
                        MapName::list_all_maps_in_city_from_manifest(&city, &Manifest::load());
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
    for city in CityName::list_all_cities_from_manifest(&Manifest::load()) {
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

    Transition::Replace(MapLoader::new_state(ctx, app, name, on_load.take().unwrap()))
}

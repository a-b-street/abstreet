use abstutil::{prettyprint_usize, MapName};
use geom::{Distance, Percent, Polygon, Pt2D};
use map_model::City;
use widgetry::{
    Autocomplete, Btn, Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome,
    Panel, ScreenPt, State, Text, TextExt, Widget,
};

use crate::app::App;
use crate::game::Transition;
use crate::helpers::{grey_out_map, nice_map_name, open_browser};
use crate::load::MapLoader;
use crate::render::DrawArea;

pub struct CityPicker {
    panel: Panel,
    // In untranslated screen-space
    regions: Vec<(MapName, Color, Polygon)>,
    selected: Option<usize>,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut App) -> Transition>>,
}

impl CityPicker {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let mut batch = GeomBatch::new();
        let mut regions = Vec::new();

        if let Ok(city) = abstutil::maybe_read_binary::<City>(
            abstutil::path(format!(
                "system/{}/city.bin",
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
        let mut more_cities = 0;
        for name in abstutil::list_all_maps() {
            if let Some((_, color, _)) = regions.iter().find(|(n, _, _)| &name == n) {
                let btn = Btn::txt(
                    abstutil::path_map(&name),
                    Text::from(Line(nice_map_name(&name)).fg(*color)),
                )
                .tooltip(Text::new());
                this_city.push(if &name == app.primary.map.get_name() {
                    btn.inactive(ctx)
                } else {
                    btn.build_def(ctx, None)
                });
            } else if other_cities.len() < 10 {
                other_cities.push(
                    Btn::txt(
                        abstutil::path_map(&name),
                        Text::from(Line(nice_map_name(&name))),
                    )
                    .tooltip(Text::new())
                    .build_def(ctx, None),
                );
            } else {
                more_cities += 1;
            }
        }
        if more_cities > 0 {
            other_cities.push(
                Btn::text_bg2(format!("{} more cities", prettyprint_usize(more_cities))).build(
                    ctx,
                    "more cities",
                    Key::Tab,
                ),
            );
        }
        if !this_city.is_empty() {
            this_city.insert(
                0,
                format!("More regions in {}", app.primary.map.get_city_name()).draw_text(ctx),
            );
        }

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
            ]))
            .build(ctx),
        })
    }
}

impl State<App> for CityPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "more cities" => {
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
                path => {
                    return Transition::Replace(MapLoader::new(
                        ctx,
                        app,
                        MapName::from_path(path),
                        self.on_load.take().unwrap(),
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
                    for (idx, (name, _, poly)) in self.regions.iter().enumerate() {
                        if name != app.primary.map.get_name() && poly.contains_pt(pt) {
                            self.selected = Some(idx);
                            break;
                        }
                    }
                } else if let Some(btn) = self.panel.currently_hovering() {
                    for (idx, (name, _, _)) in self.regions.iter().enumerate() {
                        if name != app.primary.map.get_name() && &name.map == btn {
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
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

struct AllCityPicker {
    panel: Panel,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut App) -> Transition>>,
}

impl AllCityPicker {
    fn new(
        ctx: &mut EventCtx,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State<App>> {
        let mut autocomplete_entries = Vec::new();
        let mut buttons = Vec::new();

        for name in abstutil::list_all_maps() {
            buttons.push(
                Btn::text_fg(name.describe())
                    .build(ctx, abstutil::path_map(&name), None)
                    .margin_right(10)
                    .margin_below(10),
            );
            autocomplete_entries.push((name.describe(), abstutil::path_map(&name)));
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

impl State<App> for AllCityPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                path => {
                    return Transition::Replace(MapLoader::new(
                        ctx,
                        app,
                        MapName::from_path(path),
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
                    MapName::from_path(&paths.remove(0)),
                    self.on_load.take().unwrap(),
                ));
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

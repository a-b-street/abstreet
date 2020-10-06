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
                // For example, the huge_seattle map isn't bundled in releases.
                if !abstutil::file_exists(abstutil::path_map(&name)) {
                    continue;
                }
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
        for name in abstutil::list_all_objects(abstutil::path_all_maps()) {
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
                    return ctx.loading_screen("switch map", |ctx, _| {
                        app.switch_map(ctx, abstutil::path_map(name));
                        (self.on_load)(ctx, app)
                    });
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

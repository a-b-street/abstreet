use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::nice_map_name;
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, ScreenPt, Text,
    Widget,
};
use geom::{Distance, Polygon, Pt2D};
use map_model::{AreaType, City};

// TODO Also include text buttons
// TODO Handle other cities
pub struct CityPicker {
    composite: Composite,
    // In untranslated screen-space
    regions: Vec<(String, Color, Polygon)>,
    selected: Option<usize>,
    on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
}

impl CityPicker {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        on_load: Box<dyn Fn(&mut EventCtx, &mut App) -> Transition>,
    ) -> Box<dyn State> {
        // TODO Handle if the city doesn't exist
        let city: City = abstutil::read_binary(
            format!(
                "../data/system/cities/{}.bin",
                app.primary.map.get_city_name()
            ),
            &mut abstutil::Timer::throwaway(),
        );

        let bounds = city.boundary.get_bounds();
        let zoom = (0.8 * ctx.canvas.window_width / bounds.width())
            .min(0.8 * ctx.canvas.window_height / bounds.height());

        let mut batch = GeomBatch::new();
        batch.push(app.cs.map_background, city.boundary);
        for (area_type, polygon) in city.areas {
            // TODO Refactor
            let color = match area_type {
                AreaType::Park => app.cs.grass,
                AreaType::Water => app.cs.water,
                AreaType::PedestrianIsland => Color::grey(0.3),
                AreaType::Island => app.cs.map_background,
            };
            batch.push(color, polygon);
        }

        let mut regions = Vec::new();
        for (name, polygon) in city.regions {
            let color = app.cs.rotating_color_agents(regions.len());
            batch.push(color, polygon.to_outline(Distance::meters(200.0)));
            regions.push((name, color, polygon.scale(zoom)));
        }

        Box::new(CityPicker {
            regions,
            selected: None,
            on_load,
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Click a region").small_heading().draw(ctx),
                        Btn::plaintext("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Widget::draw_batch(ctx, batch.scale(zoom)).named("picker"),
                ])
                .bg(app.cs.panel_bg)
                .outline(2.0, Color::WHITE)
                .padding(10),
            )
            .build(ctx),
        })
    }
}

impl State for CityPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if ctx.redo_mouseover() {
            self.selected = None;
            if let Some(cursor) = ctx.canvas.get_cursor_in_screen_space() {
                let rect = self.composite.rect_of("picker");
                if rect.contains(cursor) {
                    let pt = Pt2D::new(cursor.x - rect.x1, cursor.y - rect.y1);
                    for (idx, (_, _, poly)) in self.regions.iter().enumerate() {
                        if poly.contains_pt(pt) {
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
        self.composite.draw(g);

        if let Some(idx) = self.selected {
            let (name, color, poly) = &self.regions[idx];
            let rect = self.composite.rect_of("picker");
            g.fork(
                Pt2D::new(0.0, 0.0),
                ScreenPt::new(rect.x1, rect.y1),
                1.0,
                None,
            );
            g.draw_polygon(color.alpha(0.5), poly);
            g.unfork();

            g.draw_mouse_tooltip(Text::from(Line(nice_map_name(name))));
        }
    }
}

use abstutil::prettyprint_usize;
use geom::{Distance, PolyLine, Polygon, Pt2D};
use map_gui::tools::{ColorLegend, PopupMsg};
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Panel,
    SimpleState, State, Text, VerticalAlignment, Widget,
};

use crate::buildings::{BldgState, Buildings};
use crate::levels::Level;
use crate::title::TitleScreen;
use crate::{App, Transition};

const ZOOM: f64 = 2.0;

pub struct Strategize {
    unlock_messages: Option<Vec<String>>,
    draw_all: Drawable,
}

impl Strategize {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        score: usize,
        level: &Level,
        bldgs: &Buildings,
        path: RecordPath,
    ) -> Box<dyn State<App>> {
        ctx.canvas.cam_zoom = ZOOM;
        let start = app
            .map
            .get_i(app.map.find_i_by_osm_id(level.start).unwrap())
            .polygon
            .center();
        ctx.canvas.center_on_map_pt(start);

        let unlock_messages = app.session.record_score(level.title.clone(), score);

        let mut txt = Text::new();
        txt.add(Line(format!("Results for {}", level.title)).small_heading());
        txt.add(Line(format!(
            "You delivered {} presents",
            prettyprint_usize(score)
        )));
        txt.add(Line(""));
        txt.add(Line("High scores:"));
        for (idx, score) in app.session.high_scores[&level.title].iter().enumerate() {
            txt.add(Line(format!("{}) {}", idx + 1, prettyprint_usize(*score))));
        }

        // Partly duplicated with Buildings::new, but we want to label upzones and finished houses
        // differently
        let mut batch = GeomBatch::new();
        for b in app.map.all_buildings() {
            match bldgs.buildings[&b.id] {
                BldgState::Undelivered(num_housing_units) => {
                    batch.push(
                        if num_housing_units > 5 {
                            app.session.colors.apartment
                        } else {
                            app.session.colors.house
                        },
                        b.polygon.clone(),
                    );
                    if num_housing_units > 1 {
                        batch.append(
                            Text::from(Line(num_housing_units.to_string()).fg(Color::RED))
                                .render_autocropped(ctx)
                                .scale(0.2)
                                .centered_on(b.label_center),
                        );
                    }
                }
                BldgState::Store => {
                    batch.push(
                        if bldgs.upzones.contains(&b.id) {
                            Color::PINK
                        } else {
                            app.session.colors.store
                        },
                        b.polygon.clone(),
                    );
                }
                BldgState::Done => {
                    batch.push(Color::RED, b.polygon.clone());
                }
                BldgState::Ignore => {
                    batch.push(app.session.colors.visited, b.polygon.clone());
                }
            }
        }

        batch.push(Color::CYAN, path.render(Distance::meters(2.0)));

        let panel = Panel::new(Widget::col(vec![
            txt.draw(ctx),
            Btn::text_bg2("Back to title screen").build_def(ctx, Key::Enter),
            Widget::row(vec![
                ColorLegend::row(ctx, app.session.colors.house, "house"),
                ColorLegend::row(ctx, app.session.colors.apartment, "apartment"),
                ColorLegend::row(ctx, app.session.colors.store, "store"),
            ]),
            Widget::row(vec![
                ColorLegend::row(ctx, Color::PINK, "upzoned store"),
                ColorLegend::row(ctx, Color::RED, "delivered!"),
            ])
            .evenly_spaced(),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx);
        SimpleState::new(
            panel,
            Box::new(Strategize {
                unlock_messages,
                draw_all: ctx.upload(batch),
            }),
        )
    }
}

impl SimpleState<App> for Strategize {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "Back to title screen" => {
                let mut transitions = vec![
                    Transition::Pop,
                    Transition::Replace(TitleScreen::new(ctx, app)),
                ];
                if let Some(msgs) = self.unlock_messages.take() {
                    transitions.push(Transition::Push(PopupMsg::new(
                        ctx,
                        "Level complete!",
                        msgs,
                    )));
                }
                Transition::Multi(transitions)
            }
            _ => unreachable!(),
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        app.session.update_music(ctx);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw_all);
        app.session.music.draw(g);
    }
}

pub struct Results;

impl Results {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        score: usize,
        level: &Level,
    ) -> Box<dyn State<App>> {
        let mut txt = Text::new();
        if score < level.goal {
            txt.add(Line("Not quite...").small_heading());
            txt.add(Line(format!(
                "You only delivered {} / {} presents",
                prettyprint_usize(score),
                prettyprint_usize(level.goal)
            )));
            txt.add(Line("Review your route and try again."));
            txt.add(Line(""));
            txt.add(Line("Hint: look for any apartments you missed!"));
        } else {
            txt.add(Line("Thank you, Santa!").small_heading());
            txt.add(Line(format!(
                "You delivered {} presents, more than the goal of {}!",
                prettyprint_usize(score),
                prettyprint_usize(level.goal)
            )));
            let high_score = app.session.high_scores[&level.title][0];
            if high_score == score {
                txt.add(Line("Wow, a new high score!"));
            } else {
                txt.add(Line(format!(
                    "But can you beat the high score of {}?",
                    prettyprint_usize(high_score)
                )));
            }
        }

        SimpleState::new(
            Panel::new(Widget::col(vec![
                txt.draw(ctx),
                Btn::text_bg2("OK").build_def(ctx, Key::Enter),
            ]))
            .build(ctx),
            Box::new(Results),
        )
    }
}

impl SimpleState<App> for Results {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "OK" => Transition::Pop,
            _ => unreachable!(),
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.session.update_music(ctx);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.session.music.draw(g);
    }
}

pub struct RecordPath {
    pts: Vec<Pt2D>,
}

impl RecordPath {
    pub fn new() -> RecordPath {
        RecordPath { pts: Vec::new() }
    }

    pub fn add_pt(&mut self, pt: Pt2D) {
        // Do basic compression along the way
        let len = self.pts.len();
        if len >= 2 {
            let same_line = self.pts[len - 2]
                .angle_to(self.pts[len - 1])
                .approx_eq(self.pts[len - 1].angle_to(pt), 0.1);
            if same_line {
                self.pts.pop();
            }
        }

        self.pts.push(pt);
    }

    pub fn render(mut self, thickness: Distance) -> Polygon {
        self.pts.dedup();
        PolyLine::unchecked_new(self.pts).make_polygons(thickness)
    }
}

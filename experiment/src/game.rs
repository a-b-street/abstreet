use std::collections::HashMap;

use geom::{Circle, Distance, Pt2D, Speed};
use map_gui::tools::{nice_map_name, CityPicker};
use map_gui::{Cached, SimpleApp, ID};
use map_model::{BuildingID, BuildingType};
use widgetry::{
    lctrl, Btn, Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, State, Text, TextExt, Transition, UpdateType, VerticalAlignment, Widget,
};

use crate::controls::{Controller, InstantController, RotateController};

pub struct Game {
    panel: Panel,
    controls: Box<dyn Controller>,

    sleigh: Pt2D,
    score: Score,
    over_bldg: Cached<BuildingID, OverBldg>,
}

impl Game {
    pub fn new(ctx: &mut EventCtx, app: &SimpleApp) -> Box<dyn State<SimpleApp>> {
        // Start on a commerical building
        let sleigh = app
            .map
            .all_buildings()
            .into_iter()
            .find(|b| match b.bldg_type {
                BuildingType::Commercial(_) => true,
                _ => false,
            })
            .map(|b| b.label_center)
            .unwrap();
        ctx.canvas.center_on_map_pt(sleigh);

        Box::new(Game {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Experiment").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Checkbox::toggle(ctx, "control type", "rotate", "instant", Key::Tab, false),
                Widget::row(vec![Btn::pop_up(
                    ctx,
                    Some(nice_map_name(app.map.get_name())),
                )
                .build(ctx, "change map", lctrl(Key::L))]),
                format!("Score: 0").draw_text(ctx).named("score"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            controls: Box::new(InstantController::new(Speed::miles_per_hour(30.0))),

            sleigh,
            score: Score::new(ctx, app),
            over_bldg: Cached::new(),
        })
    }

    fn update_panel(&mut self, ctx: &mut EventCtx) {
        self.panel.replace(
            ctx,
            "score",
            format!("Score: {}", abstutil::prettyprint_usize(self.score.score)).draw_text(ctx),
        );
    }
}

impl State<SimpleApp> for Game {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut SimpleApp) -> Transition<SimpleApp> {
        let (dx, dy) = self.controls.displacement(ctx);
        if dx != 0.0 || dy != 0.0 {
            self.sleigh = self.sleigh.offset(dx, dy);
            ctx.canvas.center_on_map_pt(self.sleigh);

            self.over_bldg
                .update(OverBldg::key(app, self.sleigh, &self.score), |key| {
                    OverBldg::value(ctx, app, key)
                });
        }

        if let Some(b) = self.over_bldg.key() {
            if ctx.input.pressed(Key::Space) {
                if self.score.present_dropped(ctx, app, b) {
                    self.over_bldg.clear();
                    self.update_panel(ctx);
                }
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "change map" => {
                    return Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(Game::new(ctx, app)),
                            ])
                        }),
                    ));
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                self.controls = if self.panel.is_checked("control type") {
                    Box::new(RotateController::new(Speed::miles_per_hour(30.0)))
                } else {
                    Box::new(InstantController::new(Speed::miles_per_hour(30.0)))
                };
            }
            _ => {}
        }

        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &SimpleApp) {
        self.panel.draw(g);

        g.redraw(&self.score.draw_scores);
        g.redraw(&self.score.draw_done);
        if let Some(draw) = self.over_bldg.value() {
            g.redraw(&draw.0);
        }
        g.draw_polygon(
            Color::RED,
            Circle::new(self.sleigh, Distance::meters(5.0)).to_polygon(),
        );
    }
}

struct Score {
    score: usize,
    houses: HashMap<BuildingID, BldgState>,
    draw_scores: Drawable,
    draw_done: Drawable,
}

impl Score {
    fn new(ctx: &mut EventCtx, app: &SimpleApp) -> Score {
        let mut houses = HashMap::new();
        let mut batch = GeomBatch::new();
        for b in app.map.all_buildings() {
            if let BuildingType::Residential(_) = b.bldg_type {
                let score = b.id.0;
                houses.insert(b.id, BldgState::Undelivered(score));
                batch.append(
                    Text::from(Line(format!("{}", score)))
                        .render_to_batch(ctx.prerender)
                        .scale(0.1)
                        .centered_on(b.label_center),
                );
            }
        }

        Score {
            score: 0,
            houses,
            draw_scores: ctx.upload(batch),
            draw_done: Drawable::empty(ctx),
        }
    }

    fn redraw(&mut self, ctx: &mut EventCtx, app: &SimpleApp) {
        let mut batch = GeomBatch::new();
        for (b, state) in &self.houses {
            if let BldgState::Done = state {
                batch.push(Color::BLACK, app.map.get_b(*b).polygon.clone());
            }
        }
        self.draw_done = ctx.upload(batch);
    }

    // True if state change
    fn present_dropped(&mut self, ctx: &mut EventCtx, app: &SimpleApp, id: BuildingID) -> bool {
        if let Some(BldgState::Undelivered(score)) = self.houses.get(&id) {
            self.score += score;
            self.houses.insert(id, BldgState::Done);
            self.redraw(ctx, app);
            return true;
        }
        false
    }
}

enum BldgState {
    // The score ready to claim
    Undelivered(usize),
    Done,
}

struct OverBldg(Drawable);

impl OverBldg {
    fn key(app: &SimpleApp, sleigh: Pt2D, score: &Score) -> Option<BuildingID> {
        for id in app
            .draw_map
            .get_matching_objects(Circle::new(sleigh, Distance::meters(3.0)).get_bounds())
        {
            if let ID::Building(b) = id {
                if app.map.get_b(b).polygon.contains_pt(sleigh) {
                    if let Some(BldgState::Undelivered(_)) = score.houses.get(&b) {
                        return Some(b);
                    }
                }
            }
        }
        None
    }

    fn value(ctx: &mut EventCtx, app: &SimpleApp, key: BuildingID) -> OverBldg {
        OverBldg(ctx.upload(GeomBatch::from(vec![(
            Color::YELLOW,
            app.map.get_b(key).polygon.clone(),
        )])))
    }
}

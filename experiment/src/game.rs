use std::collections::HashMap;

use abstutil::{prettyprint_usize, Timer};
use geom::{Circle, Distance, Duration, Line, Polygon, Pt2D, Speed};
use map_gui::tools::{nice_map_name, CityPicker, ColorScale, SimpleMinimap};
use map_gui::{Cached, SimpleApp, ID};
use map_model::{BuildingID, BuildingType, PathConstraints};
use widgetry::{
    lctrl, Btn, Checkbox, Color, Drawable, EventCtx, Fill, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, LinearGradient, Outcome, Panel, State, Text, TextExt, Transition, UpdateType,
    VerticalAlignment, Widget,
};

use crate::animation::Animator;
use crate::controls::{Controller, InstantController, RotateController};

const ZOOM: f64 = 10.0;

pub struct Game {
    panel: Panel,
    controls: Box<dyn Controller>,
    minimap: SimpleMinimap,
    animator: Animator,

    sleigh: Pt2D,
    state: SleighState,
    over_bldg: Cached<BuildingID, OverBldg>,
}

impl Game {
    pub fn new(
        ctx: &mut EventCtx,
        app: &SimpleApp,
        timer: &mut Timer,
    ) -> Box<dyn State<SimpleApp>> {
        ctx.canvas.cam_zoom = ZOOM;

        // Start on a commerical building
        let depot = app
            .map
            .all_buildings()
            .into_iter()
            .find(|b| match b.bldg_type {
                BuildingType::Commercial(_) => true,
                _ => false,
            })
            .unwrap();
        let sleigh = depot.label_center;
        ctx.canvas.center_on_map_pt(sleigh);
        let state = SleighState::new(ctx, app, depot.id, timer);

        let panel = Panel::new(Widget::col(vec![
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
            "Score".draw_text(ctx).named("score"),
            Widget::row(vec![
                "Energy:".draw_text(ctx),
                Widget::draw_batch(ctx, GeomBatch::new())
                    .named("energy")
                    .align_right(),
            ]),
            Widget::row(vec![
                "Next upzone:".draw_text(ctx),
                Widget::draw_batch(ctx, GeomBatch::new())
                    .named("next upzone")
                    .align_right(),
            ]),
            "use upzone".draw_text(ctx).named("use upzone"),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx);
        let with_zorder = false;
        let mut game = Game {
            panel,
            controls: Box::new(InstantController::new()),
            minimap: SimpleMinimap::new(ctx, app, with_zorder),
            animator: Animator::new(ctx),

            sleigh,
            state,
            over_bldg: Cached::new(),
        };
        game.update_panel(ctx);
        Box::new(game)
    }

    fn update_panel(&mut self, ctx: &mut EventCtx) {
        self.panel.replace(
            ctx,
            "score",
            format!("Score: {}", prettyprint_usize(self.state.score)).draw_text(ctx),
        );

        let energy_bar = make_bar(
            ctx,
            self.state.energy.max(Duration::ZERO) / self.state.config.max_energy,
            ColorScale(vec![Color::RED, Color::YELLOW, Color::GREEN]),
        );
        self.panel.replace(ctx, "energy", energy_bar);

        let (upzones_free, next_upzone_pct) = self.state.get_upzones();
        self.panel.replace(
            ctx,
            "use upzone",
            if upzones_free == 0 {
                Btn::text_bg2("0 upzones").inactive(ctx).named("use upzone")
            } else {
                // TODO Since we constantly recreate this, the button isn't clickable
                Btn::text_bg2(format!("Apply upzone ({} available)", upzones_free)).build(
                    ctx,
                    "use upzone",
                    Key::U,
                )
            },
        );
        let upzone_bar = make_bar(
            ctx,
            next_upzone_pct,
            // TODO Probably similar color for showing depots
            ColorScale(vec![Color::hex("#EFEDF5"), Color::hex("#756BB1")]),
        );
        self.panel.replace(ctx, "next upzone", upzone_bar);
    }

    pub fn upzone(&mut self, ctx: &mut EventCtx, app: &SimpleApp, b: BuildingID) {
        self.state.energy = self.state.config.max_energy;
        self.state.upzones_used += 1;
        self.state.houses.insert(b, BldgState::Depot);
        self.state.depot = b;
        self.state.redraw(ctx, app);
        self.state.redraw_depots(ctx, app);
        self.sleigh = app.map.get_b(b).label_center;
        ctx.canvas.cam_zoom = ZOOM;
        ctx.canvas.center_on_map_pt(self.sleigh);
    }
}

impl State<SimpleApp> for Game {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut SimpleApp) -> Transition<SimpleApp> {
        let mut recharging = false;
        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            if let Some(b) = self.over_bldg.key() {
                if ctx.is_key_down(Key::Space) && self.state.recharge(ctx, app, b, dt) {
                    self.state.depot = b;
                    self.state.redraw(ctx, app);
                    self.update_panel(ctx);
                    recharging = true;
                }
            }

            if !recharging && self.state.has_energy() {
                self.state.energy -= dt;
                self.update_panel(ctx);
            }
        }

        if !recharging {
            let speed = if self.state.has_energy() {
                self.state.config.normal_speed
            } else {
                self.state.config.tired_speed
            };
            let (dx, dy) = self.controls.displacement(ctx, speed);
            if dx != 0.0 || dy != 0.0 {
                self.sleigh = self.sleigh.offset(dx, dy);
                ctx.canvas.center_on_map_pt(self.sleigh);

                self.over_bldg
                    .update(OverBldg::key(app, self.sleigh, &self.state), |key| {
                        OverBldg::value(ctx, app, key)
                    });
            }
        }

        if let Some(b) = self.over_bldg.key() {
            if self.state.has_energy() {
                if let Some(increase) = self.state.present_dropped(ctx, app, b) {
                    self.over_bldg.clear();
                    self.update_panel(ctx);
                    self.animator.add(
                        Duration::seconds(0.5),
                        (1.0, 4.0),
                        app.map.get_b(b).label_center,
                        Text::from(Line(format!("+{}", prettyprint_usize(increase))))
                            .bg(Color::RED)
                            .render_to_batch(ctx.prerender)
                            .scale(0.1),
                    );
                }
            }
        }

        if let Some(t) = self.minimap.event(ctx, app) {
            return t;
        }
        self.animator.event(ctx);

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
                            ctx.loading_screen("setup again", |ctx, mut timer| {
                                Transition::Multi(vec![
                                    Transition::Pop,
                                    Transition::Replace(Game::new(ctx, app, &mut timer)),
                                ])
                            })
                        }),
                    ));
                }
                "use upzone" => {
                    let choices = self
                        .state
                        .houses
                        .iter()
                        .filter_map(|(id, state)| match state {
                            BldgState::Undelivered { .. } => Some(*id),
                            _ => None,
                        })
                        .collect();
                    return Transition::Push(crate::upzone::Picker::new(ctx, app, choices));
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                self.controls = if self.panel.is_checked("control type") {
                    Box::new(RotateController::new())
                } else {
                    Box::new(InstantController::new())
                };
            }
            _ => {}
        }

        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &SimpleApp) {
        self.panel.draw(g);
        if self.state.has_energy() {
            self.minimap
                .draw_with_extra_layer(g, app, Some(&self.state.draw_done));
        } else {
            self.minimap
                .draw_with_extra_layer(g, app, Some(&self.state.draw_all_depots));
        }

        g.redraw(&self.state.draw_scores);
        g.redraw(&self.state.draw_done);
        if let Some(draw) = self.over_bldg.value() {
            g.redraw(&draw.0);
        }
        g.draw_polygon(
            Color::RED,
            Circle::new(self.sleigh, Distance::meters(5.0)).to_polygon(),
        );
        self.animator.draw(g);
    }
}

struct Config {
    normal_speed: Speed,
    tired_speed: Speed,
    recharge_rate: f64,
    max_energy: Duration,
    upzone_rate: usize,
}

struct SleighState {
    depot: BuildingID,
    score: usize,
    upzones_used: usize,
    energy: Duration,
    houses: HashMap<BuildingID, BldgState>,
    draw_scores: Drawable,
    draw_done: Drawable,
    config: Config,
    upzoned_depots: Vec<BuildingID>,
    draw_all_depots: Drawable,
}

impl SleighState {
    fn new(
        ctx: &mut EventCtx,
        app: &SimpleApp,
        depot: BuildingID,
        timer: &mut Timer,
    ) -> SleighState {
        timer.start("calculate costs from depot");
        let house_costs = map_model::connectivity::all_costs_from(
            &app.map,
            depot,
            Duration::hours(3),
            PathConstraints::Pedestrian,
        );
        timer.stop("calculate costs from depot");

        let mut houses = HashMap::new();
        let mut batch = GeomBatch::new();
        timer.start_iter("assign score to houses", app.map.all_buildings().len());
        for b in app.map.all_buildings() {
            timer.next();
            if let BuildingType::Residential(_) = b.bldg_type {
                let score = b.id.0;
                let cost = house_costs.get(&b.id).cloned().unwrap_or(Duration::ZERO);
                let color = if cost < Duration::minutes(5) {
                    Color::GREEN
                } else if cost < Duration::minutes(15) {
                    Color::YELLOW
                } else {
                    Color::RED
                };

                houses.insert(b.id, BldgState::Undelivered { score, cost });
                // TODO Very expensive
                batch.append(
                    Text::from_multiline(vec![
                        Line(format!("{}", score)),
                        Line(format!("{}", cost)).fg(color),
                    ])
                    .render_to_batch(ctx.prerender)
                    .scale(0.1)
                    .centered_on(b.label_center),
                );
            } else if !b.amenities.is_empty() {
                // TODO Maybe just food?
                houses.insert(b.id, BldgState::Depot);
            }
        }

        let config = Config {
            normal_speed: Speed::miles_per_hour(30.0),
            tired_speed: Speed::miles_per_hour(10.0),
            recharge_rate: 1000.0,
            max_energy: Duration::minutes(90),
            upzone_rate: 30_000,
        };
        let mut s = SleighState {
            depot,
            score: 0,
            upzones_used: 0,
            energy: config.max_energy,
            houses,
            draw_scores: ctx.upload(batch),
            draw_done: Drawable::empty(ctx),
            config,
            upzoned_depots: Vec::new(),
            draw_all_depots: Drawable::empty(ctx),
        };
        s.redraw(ctx, app);
        s.redraw_depots(ctx, app);
        s
    }

    fn redraw(&mut self, ctx: &mut EventCtx, app: &SimpleApp) {
        let mut batch = GeomBatch::new();
        for (b, state) in &self.houses {
            if let BldgState::Done = state {
                batch.push(Color::BLACK, app.map.get_b(*b).polygon.clone());
            }
        }
        // TODO This doesnt seem to be working
        for b in &self.upzoned_depots {
            batch.push(
                app.cs.commerical_building,
                app.map.get_b(*b).polygon.clone(),
            );
        }
        batch.push(Color::GREEN, app.map.get_b(self.depot).polygon.clone());
        self.draw_done = ctx.upload(batch);
    }

    fn redraw_depots(&mut self, ctx: &mut EventCtx, app: &SimpleApp) {
        let mut batch = GeomBatch::new();
        for (b, state) in &self.houses {
            if let BldgState::Depot = state {
                batch.push(Color::RED, app.map.get_b(*b).polygon.clone());
            }
        }
        self.draw_all_depots = ctx.upload(batch);
    }

    // If something changed, return the update to the score
    fn present_dropped(
        &mut self,
        ctx: &mut EventCtx,
        app: &SimpleApp,
        id: BuildingID,
    ) -> Option<usize> {
        if let Some(BldgState::Undelivered { score, cost }) = self.houses.get(&id).cloned() {
            self.score += score;
            self.houses.insert(id, BldgState::Done);
            self.energy -= cost;
            self.redraw(ctx, app);
            return Some(score);
        }
        None
    }

    // True if state change
    fn recharge(
        &mut self,
        ctx: &mut EventCtx,
        app: &SimpleApp,
        id: BuildingID,
        dt: Duration,
    ) -> bool {
        if let Some(BldgState::Depot) = self.houses.get(&id) {
            self.energy += self.config.recharge_rate * dt;
            self.energy = self.energy.min(self.config.max_energy);
            self.redraw(ctx, app);
            return true;
        }
        false
    }

    fn has_energy(&self) -> bool {
        self.energy > Duration::ZERO
    }

    /// (upzones_free, next_upzone_pct)
    fn get_upzones(&self) -> (usize, f64) {
        // Start with a freebie
        let total = 1 + (self.score / self.config.upzone_rate);
        let upzones_free = total - self.upzones_used;
        let next_upzone = total * self.config.upzone_rate;
        (
            upzones_free,
            1.0 - ((next_upzone - self.score) as f64) / (self.config.upzone_rate as f64),
        )
    }
}

#[derive(Clone)]
enum BldgState {
    Undelivered { score: usize, cost: Duration },
    Depot,
    Done,
}

struct OverBldg(Drawable);

impl OverBldg {
    fn key(app: &SimpleApp, sleigh: Pt2D, state: &SleighState) -> Option<BuildingID> {
        for id in app
            .draw_map
            .get_matching_objects(Circle::new(sleigh, Distance::meters(3.0)).get_bounds())
        {
            if let ID::Building(b) = id {
                if app.map.get_b(b).polygon.contains_pt(sleigh) {
                    if let Some(BldgState::Undelivered { .. }) | Some(BldgState::Depot) =
                        state.houses.get(&b)
                    {
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

fn make_bar(ctx: &mut EventCtx, pct_full: f64, scale: ColorScale) -> Widget {
    let total_width = 300.0;
    let height = 32.0;
    let n = scale.0.len();
    let width_each = total_width / ((n - 1) as f64);

    let mut pieces = Vec::new();
    let mut width_remaining = pct_full * total_width;
    for i in 0..n - 1 {
        let width = width_each.min(width_remaining);
        pieces.push(Polygon::rectangle(width, height).translate((i as f64) * width_each, 0.0));
        if width < width_each {
            break;
        }
        width_remaining -= width;
    }

    let mut batch = GeomBatch::new();
    batch.push(
        Fill::LinearGradient(LinearGradient {
            line: Line::must_new(Pt2D::new(0.0, 0.0), Pt2D::new(total_width, 0.0)),
            stops: scale
                .0
                .iter()
                .enumerate()
                .map(|(idx, color)| ((idx as f64) / ((n - 1) as f64), *color))
                .collect(),
        }),
        Polygon::union_all(pieces),
    );
    batch.push(
        Color::BLACK,
        Polygon::rectangle((1.0 - pct_full) * total_width, height)
            .translate(pct_full * total_width, 0.0),
    );
    Widget::draw_batch(ctx, batch)
        .padding(2)
        .outline(2.0, Color::WHITE)
}

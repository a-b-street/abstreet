use std::collections::HashSet;

use abstutil::prettyprint_usize;
use geom::{ArrowCap, Circle, Distance, Duration, PolyLine, Pt2D, Time};
use map_gui::tools::{ChooseSomething, ColorLegend, Minimap, MinimapControls};
use map_model::BuildingID;
use widgetry::{
    Btn, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, State, Text, TextExt, UpdateType, VerticalAlignment, Widget,
};

use crate::after_level::Results;
use crate::animation::{Animator, Effect, SnowEffect};
use crate::buildings::{BldgState, Buildings};
use crate::levels::Level;
use crate::meters::{custom_bar, make_bar};
use crate::movement::Player;
use crate::vehicles::Vehicle;
use crate::{App, Transition};

const ACQUIRE_BOOST_RATE: f64 = 0.5;
const BOOST_SPEED_MULTIPLIER: f64 = 2.0;

pub struct Game {
    title_panel: Panel,
    status_panel: Panel,
    time_panel: Panel,
    boost_panel: Panel,
    pause_panel: Panel,
    minimap: Minimap<App, MinimapController>,

    animator: Animator,
    snow: SnowEffect,

    state: GameState,
    player: Player,
}

impl Game {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        level: Level,
        vehicle: Vehicle,
        upzones: HashSet<BuildingID>,
    ) -> Box<dyn State<App>> {
        app.session.current_vehicle = vehicle.name.clone();
        app.time = Time::START_OF_DAY;

        let title_panel = Panel::new(Widget::row(vec![
            "15 min Santa".draw_text(ctx).centered_vert(),
            Widget::row(vec![
                // TODO The blur is messed up
                Widget::draw_svg(ctx, "system/assets/tools/map.svg").centered_vert(),
                Line(&level.title).draw(ctx),
            ])
            .padding(10)
            .bg(Color::hex("#003046")),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::TopInset)
        .build(ctx);

        let status_panel = Panel::new(Widget::col(vec![
            "Complete Deliveries".draw_text(ctx).named("score label"),
            Widget::draw_batch(ctx, GeomBatch::new()).named("score"),
            "Remaining Gifts".draw_text(ctx),
            Widget::draw_batch(ctx, GeomBatch::new()).named("energy"),
        ]))
        .aligned(HorizontalAlignment::RightInset, VerticalAlignment::TopInset)
        .build(ctx);

        let time_panel = Panel::new(Widget::row(vec![
            Widget::draw_batch(ctx, GeomBatch::new()).named("time circle"),
            "Time".draw_text(ctx).named("time label"),
        ]))
        .aligned(HorizontalAlignment::LeftInset, VerticalAlignment::TopInset)
        .build(ctx);

        let boost_panel = Panel::new(Widget::row(vec![
            "Boost".draw_text(ctx),
            Widget::draw_batch(ctx, GeomBatch::new())
                .named("boost")
                .align_right(),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::BottomInset)
        .build(ctx);

        let pause_panel = Panel::new(
            Btn::svg_def("system/assets/speed/pause_v2.svg")
                .build(ctx, "pause", Key::Escape)
                .container(),
        )
        // TODO Very brittle layout to wind up to the right of the volume panel...
        .aligned(
            HorizontalAlignment::Percent(0.05),
            VerticalAlignment::BottomInset,
        )
        .build(ctx);

        let start = app
            .map
            .find_i_by_osm_id(level.start)
            .expect(&format!("can't find {}", level.start));
        let player = Player::new(ctx, app, start);

        let bldgs = Buildings::new(ctx, app, upzones);
        let state = GameState::new(ctx, app, level, vehicle, bldgs);

        let mut game = Game {
            title_panel,
            status_panel,
            time_panel,
            boost_panel,
            pause_panel,
            minimap: Minimap::new(ctx, app, MinimapController),

            animator: Animator::new(ctx),
            snow: SnowEffect::new(ctx),

            state,
            player,
        };
        game.update_time_panel(ctx, app);
        game.update_status_panel(ctx, app);
        game.update_boost_panel(ctx, app);
        game.minimap
            .set_zoom(ctx, app, game.state.level.minimap_zoom);
        Box::new(game)
    }

    fn update_time_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        let pct = (app.time - Time::START_OF_DAY) / self.state.level.time_limit;

        let text_color = if pct < 0.75 { Color::WHITE } else { Color::RED };
        let label = Line(format!(
            "{}",
            self.state.level.time_limit - (app.time - Time::START_OF_DAY)
        ))
        .fg(text_color)
        .draw(ctx)
        .centered_vert();
        self.time_panel.replace(ctx, "time label", label);

        // TODO I couldn't quite work out how to get the partial outline from Figma working
        let center = Pt2D::new(0.0, 0.0);
        let outer = Distance::meters(15.0);
        let draw = Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![
                (Color::WHITE, Circle::new(center, outer).to_polygon()),
                (
                    Color::hex("#5D92C2"),
                    Circle::new(center, outer).to_partial_polygon(pct),
                ),
            ])
            .autocrop(),
        );
        self.time_panel.replace(ctx, "time circle", draw);
    }

    fn update_status_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        let score_bar = make_bar(
            ctx,
            app.session.colors.score,
            self.state.score,
            if self.state.met_goal() {
                self.state.bldgs.total_housing_units
            } else {
                self.state.level.goal
            },
        );
        self.status_panel.replace(ctx, "score", score_bar);

        let energy_bar = make_bar(
            ctx,
            app.session.colors.energy,
            self.state.energy,
            self.state.vehicle.max_energy,
        );
        self.status_panel.replace(ctx, "energy", energy_bar);
    }

    fn update_boost_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        let boost_bar = custom_bar(
            ctx,
            app.session.colors.boost,
            self.state.boost / self.state.vehicle.max_boost,
            if self.state.boost == Duration::ZERO {
                Text::from(Line("Find a bike or bus lane"))
            } else {
                Text::from(Line("Hold space to boost"))
            },
        );
        self.boost_panel.replace(ctx, "boost", boost_bar);
    }
}

impl State<App> for Game {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let orig_boost = self.state.boost;
        let (orig_score, orig_energy) = (self.state.score, self.state.energy);

        // Most things depend on time passing and don't care about other events
        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            app.time += dt;

            if app.time - Time::START_OF_DAY >= self.state.level.time_limit {
                return Transition::Replace(Results::new(
                    ctx,
                    app,
                    self.state.score,
                    &self.state.level,
                ));
            }

            self.update_time_panel(ctx, app);

            let base_speed = if self.state.has_energy() {
                self.state.vehicle.normal_speed
            } else {
                self.state.vehicle.tired_speed
            };
            let speed = if ctx.is_key_down(Key::Space) && self.state.boost > Duration::ZERO {
                if !self.player.on_good_road(app) {
                    self.state.boost -= dt;
                    self.state.boost = self.state.boost.max(Duration::ZERO);
                }
                base_speed * BOOST_SPEED_MULTIPLIER
            } else {
                base_speed
            };

            let met_goal = self.state.met_goal();
            for b in self.player.update_with_speed(ctx, app, speed) {
                match self.state.bldgs.buildings[&b] {
                    BldgState::Undelivered(_) => {
                        if let Some(increase) = self.state.present_dropped(ctx, app, b) {
                            let path_speed = Duration::seconds(0.2);
                            self.animator.add(
                                app.time,
                                path_speed,
                                Effect::FollowPath {
                                    color: app.session.colors.score,
                                    width: map_model::NORMAL_LANE_THICKNESS,
                                    pl: app.map.get_b(b).driveway_geom.reversed(),
                                },
                            );
                            self.animator.add(
                                app.time + path_speed,
                                Duration::seconds(0.5),
                                Effect::Scale {
                                    lerp_scale: (1.0, 4.0),
                                    center: app.map.get_b(b).label_center,
                                    orig: Text::from(Line(format!(
                                        "+{}",
                                        prettyprint_usize(increase)
                                    )))
                                    .bg(app.session.colors.score)
                                    .render_to_batch(ctx.prerender)
                                    .scale(0.1),
                                },
                            );
                        }
                    }
                    BldgState::Store => {
                        let refill = self.state.vehicle.max_energy - self.state.energy;
                        if refill > 0 {
                            self.state.energy += refill;
                            let path_speed = Duration::seconds(0.2);
                            self.animator.add(
                                app.time,
                                path_speed,
                                Effect::FollowPath {
                                    color: app.session.colors.energy,
                                    width: map_model::NORMAL_LANE_THICKNESS,
                                    pl: app.map.get_b(b).driveway_geom.clone(),
                                },
                            );
                            self.animator.add(
                                app.time + path_speed,
                                Duration::seconds(0.5),
                                Effect::Scale {
                                    lerp_scale: (1.0, 4.0),
                                    center: app.map.get_b(b).label_center,
                                    orig: Text::from(Line(format!(
                                        "Refilled {}",
                                        prettyprint_usize(refill)
                                    )))
                                    .bg(app.session.colors.energy)
                                    .render_to_batch(ctx.prerender)
                                    .scale(0.1),
                                },
                            );
                        }
                    }
                    BldgState::Done => {}
                }
            }
            if !met_goal && self.state.met_goal() {
                // TODO What should we say here? Should we add some kind of animation to call this
                // out?
                let label = "Goal met! Keep going".draw_text(ctx);
                self.status_panel.replace(ctx, "score label", label);
            }

            if self.player.on_good_road(app) && !ctx.is_key_down(Key::Space) {
                self.state.boost += dt * ACQUIRE_BOOST_RATE;
                self.state.boost = self.state.boost.min(self.state.vehicle.max_boost);
            }

            self.animator.event(ctx, app.time);
            self.snow.event(ctx, app.time);
            if self.state.has_energy() {
                self.state.energyless_arrow = None;
            } else {
                if self.state.energyless_arrow.is_none() {
                    self.state.energyless_arrow = Some(EnergylessArrow::new(ctx, app.time));
                }
                let stores = self.state.bldgs.all_stores();
                self.state.energyless_arrow.as_mut().unwrap().update(
                    ctx,
                    app,
                    self.player.get_pos(),
                    stores,
                );
            }

            if self.state.boost != orig_boost {
                self.update_boost_panel(ctx, app);
            }
            if self.state.score != orig_score || self.state.energy != orig_energy {
                self.update_status_panel(ctx, app);
            }
        } else {
            if let Some(t) = self.minimap.event(ctx, app) {
                return t;
            }

            match self.pause_panel.event(ctx) {
                Outcome::Clicked(x) => match x.as_ref() {
                    "pause" => {
                        return Transition::Push(ChooseSomething::new(
                            ctx,
                            "Game Paused",
                            vec![
                                Choice::string("Resume").key(Key::Escape),
                                Choice::string("Quit"),
                            ],
                            Box::new(|resp, _, _| match resp.as_ref() {
                                "Resume" => Transition::Pop,
                                "Quit" => Transition::Multi(vec![Transition::Pop, Transition::Pop]),
                                _ => unreachable!(),
                            }),
                        ));
                    }
                    _ => unreachable!(),
                },
                _ => {}
            }

            if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                ctx.canvas.cam_zoom = 1.1_f64
                    .powf(ctx.canvas.cam_zoom.log(1.1) + dy)
                    .max(app.opts.min_zoom_for_detail)
                    .min(50.0);
                ctx.canvas.center_on_map_pt(self.player.get_pos());
            }

            app.session.update_music(ctx);
        }

        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.title_panel.draw(g);
        self.status_panel.draw(g);
        self.time_panel.draw(g);
        self.boost_panel.draw(g);
        self.pause_panel.draw(g);
        app.session.music.draw(g);

        let santa_tracker = g.upload(GeomBatch::from(vec![(
            Color::RED,
            Circle::new(self.player.get_pos(), Distance::meters(20.0)).to_polygon(),
        )]));
        self.minimap.draw_with_extra_layers(
            g,
            app,
            vec![
                &self.state.bldgs.draw_all,
                &self.state.draw_done_houses,
                &santa_tracker,
            ],
        );

        g.redraw(&self.state.bldgs.draw_all);
        g.redraw(&self.state.draw_done_houses);

        if true {
            self.state
                .vehicle
                .animate(g, app.time)
                .centered_on(self.player.get_pos())
                .rotate_around_batch_center(self.player.get_angle())
                .draw(g);
        } else {
            // Debug
            g.draw_polygon(
                Color::RED,
                Circle::new(self.player.get_pos(), Distance::meters(2.0)).to_polygon(),
            );
        }

        self.snow.draw(g);
        self.animator.draw(g);
        if let Some(ref arrow) = self.state.energyless_arrow {
            g.redraw(&arrow.draw);
        }
    }
}

struct GameState {
    level: Level,
    vehicle: Vehicle,
    bldgs: Buildings,

    // Number of deliveries
    score: usize,
    // Number of gifts currently being carried
    energy: usize,
    boost: Duration,

    draw_done_houses: Drawable,
    energyless_arrow: Option<EnergylessArrow>,
}

impl GameState {
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        level: Level,
        vehicle: Vehicle,
        bldgs: Buildings,
    ) -> GameState {
        let energy = vehicle.max_energy;
        let mut s = GameState {
            level,
            vehicle,
            bldgs,

            score: 0,
            energy,
            boost: Duration::ZERO,

            draw_done_houses: Drawable::empty(ctx),
            energyless_arrow: None,
        };
        s.recalc_deliveries(ctx, app);
        s
    }

    fn recalc_deliveries(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut batch = GeomBatch::new();
        for (b, state) in &self.bldgs.buildings {
            if let BldgState::Done = state {
                batch.push(
                    app.session.colors.visited,
                    app.map.get_b(*b).polygon.clone(),
                );
            }
        }
        self.draw_done_houses = ctx.upload(batch);
    }

    // If something changed, return the update to the score
    fn present_dropped(&mut self, ctx: &mut EventCtx, app: &App, id: BuildingID) -> Option<usize> {
        if !self.has_energy() {
            return None;
        }
        if let BldgState::Undelivered(num_housing_units) = self.bldgs.buildings[&id] {
            // TODO No partial deliveries.
            let deliveries = num_housing_units.min(self.energy);
            self.score += deliveries;
            self.bldgs.buildings.insert(id, BldgState::Done);
            self.energy -= deliveries;
            self.recalc_deliveries(ctx, app);
            return Some(deliveries);
        }
        None
    }

    fn has_energy(&self) -> bool {
        self.energy > 0
    }

    fn met_goal(&self) -> bool {
        self.score >= self.level.goal
    }
}

struct EnergylessArrow {
    draw: Drawable,
    started: Time,
    last_update: Time,
}

impl EnergylessArrow {
    fn new(ctx: &EventCtx, started: Time) -> EnergylessArrow {
        EnergylessArrow {
            draw: Drawable::empty(ctx),
            started,
            last_update: Time::START_OF_DAY,
        }
    }

    fn update(&mut self, ctx: &mut EventCtx, app: &App, sleigh: Pt2D, all_stores: Vec<BuildingID>) {
        if self.last_update == app.time {
            return;
        }
        self.last_update = app.time;
        // Find the closest store as the crow -- or Santa -- flies
        // TODO Or pathfind and show them that?
        let store = app.map.get_b(
            all_stores
                .into_iter()
                .min_by_key(|b| app.map.get_b(*b).label_center.fast_dist(sleigh))
                .unwrap(),
        );

        // Vibrate in size slightly
        let period = Duration::seconds(0.5);
        let pct = ((app.time - self.started) % period) / period;
        // -1 to 1
        let shift = (pct * std::f64::consts::PI).sin();
        let thickness = Distance::meters(5.0 + shift);

        let angle = sleigh.angle_to(store.label_center);
        let arrow = PolyLine::must_new(vec![
            sleigh.project_away(Distance::meters(20.0), angle),
            sleigh.project_away(Distance::meters(40.0), angle),
        ])
        .make_arrow(thickness, ArrowCap::Triangle);
        self.draw = ctx.upload(GeomBatch::from(vec![(Color::RED.alpha(0.8), arrow)]));
    }
}

struct MinimapController;

impl MinimapControls<App> for MinimapController {
    fn has_zorder(&self, _: &App) -> bool {
        false
    }

    fn make_legend(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        Widget::row(vec![
            ColorLegend::row(ctx, app.session.colors.house, "house"),
            ColorLegend::row(ctx, app.session.colors.apartment, "apartment"),
            ColorLegend::row(ctx, app.session.colors.store, "store"),
        ])
        .evenly_spaced()
    }
}

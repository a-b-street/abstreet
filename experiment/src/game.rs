use std::collections::HashMap;

use abstutil::prettyprint_usize;
use geom::{ArrowCap, Circle, Distance, Duration, PolyLine, Pt2D, Time};
use map_gui::load::MapLoader;
use map_gui::tools::{ColorScale, SimpleMinimap};
use map_gui::{SimpleApp, ID};
use map_model::{BuildingID, BuildingType};
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, Text, TextExt, Transition, UpdateType, VerticalAlignment, Widget,
};

use crate::animation::{Animator, SnowEffect};
use crate::controls::InstantController;
use crate::levels::Config;
use crate::meters::make_bar;

const ZOOM: f64 = 10.0;

pub struct Game {
    panel: Panel,
    controls: InstantController,
    minimap: SimpleMinimap,
    animator: Animator,
    snow: SnowEffect,

    time: Time,
    sleigh: Pt2D,
    state: SleighState,
    over_bldg: Option<BuildingID>,
}

impl Game {
    pub fn new(ctx: &mut EventCtx, app: &SimpleApp, config: Config) -> Box<dyn State<SimpleApp>> {
        MapLoader::new(
            ctx,
            app,
            config.map.clone(),
            Box::new(move |ctx, app| {
                let start = app
                    .map
                    .find_b_by_osm_id(config.start_depot)
                    .expect(&format!("can't find {}", config.start_depot));
                let sleigh = app.map.get_b(start).label_center;
                ctx.canvas.center_on_map_pt(sleigh);
                ctx.canvas.cam_zoom = ZOOM;

                let state = SleighState::new(ctx, app, config);

                let panel = Panel::new(Widget::col(vec![
                    Widget::row(vec![
                        Line("15-minute Santa").small_heading().draw(ctx),
                        Btn::close(ctx),
                    ]),
                    Widget::row(vec![
                        "Time spent:".draw_text(ctx),
                        Widget::draw_batch(ctx, GeomBatch::new())
                            .named("time")
                            .align_right(),
                    ]),
                    Widget::row(vec![
                        "Deliveries completed:".draw_text(ctx),
                        Widget::draw_batch(ctx, GeomBatch::new())
                            .named("score")
                            .align_right(),
                    ]),
                    Widget::row(vec![
                        "Presents in bag:".draw_text(ctx),
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
                    controls: InstantController::new(),
                    minimap: SimpleMinimap::new(ctx, app, with_zorder),
                    animator: Animator::new(ctx),
                    snow: SnowEffect::new(ctx),

                    time: Time::START_OF_DAY,
                    sleigh,
                    state,
                    over_bldg: None,
                };
                game.update_panel(ctx);
                game.minimap
                    .set_zoom(ctx, app, game.state.config.minimap_zoom);
                Transition::Replace(Box::new(game))
            }),
        )
    }

    fn update_panel(&mut self, ctx: &mut EventCtx) {
        let time = format!("{}", self.time - Time::START_OF_DAY).draw_text(ctx);
        self.panel.replace(ctx, "time", time);

        let score_bar = make_bar(
            ctx,
            ColorScale(vec![Color::WHITE, Color::GREEN]),
            self.state.score,
            self.state.total_housing_units,
        );
        self.panel.replace(ctx, "score", score_bar);

        let energy_bar = make_bar(
            ctx,
            ColorScale(vec![Color::RED, Color::YELLOW, Color::GREEN]),
            self.state.energy,
            self.state.config.max_energy,
        );
        self.panel.replace(ctx, "energy", energy_bar);

        let (upzones_free, have_towards_next, needed_total) = self.state.get_upzones();
        self.panel.replace(
            ctx,
            "use upzone",
            if upzones_free == 0 {
                Btn::text_bg2("0 upzones").inactive(ctx).named("use upzone")
            } else {
                // TODO Since we constantly recreate this, the button isn't clickable
                Btn::text_bg2(format!(
                    "Apply upzone ({} available) -- press the U key",
                    upzones_free
                ))
                .build(ctx, "use upzone", Key::U)
            },
        );
        let upzone_bar = make_bar(
            ctx,
            // TODO Probably similar color for showing depots
            ColorScale(vec![Color::hex("#EFEDF5"), Color::hex("#756BB1")]),
            have_towards_next,
            needed_total,
        );
        self.panel.replace(ctx, "next upzone", upzone_bar);
    }

    pub fn upzone(&mut self, ctx: &mut EventCtx, app: &SimpleApp, b: BuildingID) {
        self.state.energy = self.state.config.max_energy;
        self.state.upzones_used += 1;
        self.state.houses.insert(b, BldgState::Depot);
        self.state.recalc_depots(ctx, app);
        self.sleigh = app.map.get_b(b).label_center;
        ctx.canvas.cam_zoom = ZOOM;
        ctx.canvas.center_on_map_pt(self.sleigh);
    }

    fn over_bldg(&self, app: &SimpleApp) -> Option<BuildingID> {
        for id in app
            .draw_map
            .get_matching_objects(Circle::new(self.sleigh, Distance::meters(3.0)).get_bounds())
        {
            if let ID::Building(b) = id {
                if app.map.get_b(b).polygon.contains_pt(self.sleigh) {
                    return Some(b);
                }
            }
        }
        None
    }
}

impl State<SimpleApp> for Game {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut SimpleApp) -> Transition<SimpleApp> {
        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            self.time += dt;
        }

        let speed = if self.state.has_energy() {
            self.state.config.normal_speed
        } else {
            self.state.config.tired_speed
        };
        let (dx, dy) = self.controls.displacement(ctx, speed);
        if dx != 0.0 || dy != 0.0 {
            self.sleigh = self.sleigh.offset(dx, dy);
            ctx.canvas.center_on_map_pt(self.sleigh);
            self.over_bldg = self.over_bldg(app);
        }

        if let Some(b) = self.over_bldg {
            match self.state.houses.get(&b) {
                Some(BldgState::Undelivered(_)) => {
                    if let Some(increase) = self.state.present_dropped(ctx, app, b) {
                        self.animator.add(
                            self.time,
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
                Some(BldgState::Depot) => {
                    let refill = self.state.config.max_energy - self.state.energy;
                    if refill > 0 {
                        self.state.energy += refill;
                        self.animator.add(
                            self.time,
                            Duration::seconds(0.5),
                            (1.0, 4.0),
                            app.map.get_b(b).label_center,
                            Text::from(Line(format!("Refilled {}", prettyprint_usize(refill))))
                                .bg(Color::BLUE)
                                .render_to_batch(ctx.prerender)
                                .scale(0.1),
                        );
                    }
                }
                _ => {}
            }
        }

        if let Some(t) = self.minimap.event(ctx, app) {
            return t;
        }
        self.animator.event(ctx, self.time);
        self.snow.event(ctx, self.time);
        if self.state.has_energy() {
            self.state.energyless_arrow = None;
        } else {
            if self.state.energyless_arrow.is_none() {
                self.state.energyless_arrow = Some(EnergylessArrow::new(ctx, self.time));
            }
            let depots = self.state.all_depots();
            self.state.energyless_arrow.as_mut().unwrap().update(
                ctx,
                app,
                self.time,
                self.sleigh,
                depots,
            );
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "use upzone" => {
                    let choices = self
                        .state
                        .houses
                        .iter()
                        .filter_map(|(id, state)| match state {
                            BldgState::Undelivered(_) => Some(*id),
                            _ => None,
                        })
                        .collect();
                    return Transition::Push(crate::upzone::Picker::new(ctx, app, choices));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        // Time is constantly passing
        self.update_panel(ctx);
        ctx.request_update(UpdateType::Game);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &SimpleApp) {
        self.panel.draw(g);
        if self.state.has_energy() {
            self.minimap.draw_with_extra_layers(
                g,
                app,
                vec![&self.state.draw_todo_houses, &self.state.draw_done_houses],
            );
        } else {
            self.minimap
                .draw_with_extra_layers(g, app, vec![&self.state.draw_all_depots]);
        }

        g.redraw(&self.state.draw_todo_houses);
        g.redraw(&self.state.draw_done_houses);
        if !self.state.has_energy() {
            g.redraw(&self.state.draw_all_depots);
        }

        GeomBatch::load_svg(g.prerender, "system/assets/characters/santa.svg")
            .scale(0.1)
            .centered_on(self.sleigh)
            .rotate_around_batch_center(self.controls.facing)
            .draw(g);
        self.snow.draw(g);
        self.animator.draw(g);
        if let Some(ref arrow) = self.state.energyless_arrow {
            g.redraw(&arrow.draw);
        }
    }
}

struct SleighState {
    config: Config,

    total_housing_units: usize,

    // Number of deliveries
    score: usize,
    // Number of gifts currently being carried
    energy: usize,
    houses: HashMap<BuildingID, BldgState>,

    upzones_used: usize,
    upzoned_depots: Vec<BuildingID>,

    draw_all_depots: Drawable,
    // This gets covered up by draw_done_houses, instead of an expensive update
    draw_todo_houses: Drawable,
    draw_done_houses: Drawable,
    energyless_arrow: Option<EnergylessArrow>,
}

impl SleighState {
    fn new(ctx: &mut EventCtx, app: &SimpleApp, config: Config) -> SleighState {
        let mut houses = HashMap::new();
        let mut total_housing_units = 0;
        for b in app.map.all_buildings() {
            if let BuildingType::Residential {
                num_housing_units, ..
            } = b.bldg_type
            {
                // There are some unused commercial buildings around!
                if num_housing_units > 0 {
                    houses.insert(b.id, BldgState::Undelivered(num_housing_units));
                    total_housing_units += num_housing_units;
                }
            } else if !b.amenities.is_empty() {
                // TODO Maybe just food?
                houses.insert(b.id, BldgState::Depot);
            }
        }

        let energy = config.max_energy;
        let mut s = SleighState {
            config,

            total_housing_units,

            score: 0,
            energy,
            houses,

            upzones_used: 0,
            upzoned_depots: Vec::new(),

            draw_all_depots: Drawable::empty(ctx),
            draw_todo_houses: Drawable::empty(ctx),
            draw_done_houses: Drawable::empty(ctx),
            energyless_arrow: None,
        };

        s.recalc_depots(ctx, app);
        s.recalc_deliveries(ctx, app);

        s
    }

    fn recalc_depots(&mut self, ctx: &mut EventCtx, app: &SimpleApp) {
        let mut batch = GeomBatch::new();
        for b in &self.upzoned_depots {
            batch.push(
                app.cs.commerical_building,
                app.map.get_b(*b).polygon.clone(),
            );
        }

        for b in app.map.all_buildings() {
            match self.houses.get(&b.id) {
                Some(BldgState::Undelivered(housing_units)) => {
                    // Call out non-single family homes
                    if *housing_units > 1 {
                        // TODO Text can be slow to render, and this should be louder anyway
                        batch.append(
                            Text::from(Line(housing_units.to_string()).fg(Color::RED))
                                .render_to_batch(ctx.prerender)
                                .scale(0.2)
                                .centered_on(b.label_center),
                        );
                    }
                    continue;
                }
                Some(BldgState::Depot) => continue,
                _ => {}
            }
            // If the house isn't reachable at all or it's not a depot or residence, just blank it
            // out
            batch.push(Color::BLACK, b.polygon.clone());
        }

        self.draw_todo_houses = ctx.upload(batch);

        // Now highlight all depots for when we run out
        let mut batch = GeomBatch::new();
        for b in self.all_depots() {
            batch.push(Color::YELLOW, app.map.get_b(b).polygon.clone());
        }
        self.draw_all_depots = ctx.upload(batch);
    }

    fn recalc_deliveries(&mut self, ctx: &mut EventCtx, app: &SimpleApp) {
        let mut batch = GeomBatch::new();
        for (b, state) in &self.houses {
            if let BldgState::Done = state {
                batch.push(Color::BLACK, app.map.get_b(*b).polygon.clone());
            }
        }
        self.draw_done_houses = ctx.upload(batch);
    }

    // If something changed, return the update to the score
    fn present_dropped(
        &mut self,
        ctx: &mut EventCtx,
        app: &SimpleApp,
        id: BuildingID,
    ) -> Option<usize> {
        if !self.has_energy() {
            return None;
        }
        if let Some(BldgState::Undelivered(num_housing_units)) = self.houses.get(&id).cloned() {
            // TODO No partial deliveries.
            let deliveries = num_housing_units.min(self.energy);
            self.score += deliveries;
            self.houses.insert(id, BldgState::Done);
            self.energy -= deliveries;
            self.recalc_deliveries(ctx, app);
            return Some(deliveries);
        }
        None
    }

    fn has_energy(&self) -> bool {
        self.energy > 0
    }

    /// (upzones_free, points towards next upzone, points needed for next upzone)
    fn get_upzones(&self) -> (usize, usize, usize) {
        // Start with a freebie
        let total = 1 + (self.score / self.config.upzone_rate);
        let upzones_free = total - self.upzones_used;
        let next_upzone = total * self.config.upzone_rate;
        (
            upzones_free,
            self.score - (next_upzone - self.config.upzone_rate),
            self.config.upzone_rate,
        )
    }

    fn all_depots(&self) -> Vec<BuildingID> {
        let mut depots = self.upzoned_depots.clone();
        for (b, state) in &self.houses {
            if let BldgState::Depot = state {
                depots.push(*b);
            }
        }
        depots
    }
}

#[derive(Clone)]
enum BldgState {
    // Score
    Undelivered(usize),
    Depot,
    Done,
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

    fn update(
        &mut self,
        ctx: &mut EventCtx,
        app: &SimpleApp,
        time: Time,
        sleigh: Pt2D,
        all_depots: Vec<BuildingID>,
    ) {
        if self.last_update == time {
            return;
        }
        self.last_update = time;
        // Find the closest depot as the crow -- or Santa -- flies
        let depot = app.map.get_b(
            all_depots
                .into_iter()
                .min_by_key(|b| app.map.get_b(*b).label_center.fast_dist(sleigh))
                .unwrap(),
        );

        // Vibrate in size slightly
        let period = Duration::seconds(0.5);
        let pct = ((time - self.started) % period) / period;
        // -1 to 1
        let shift = (pct * std::f64::consts::PI).sin();
        let thickness = Distance::meters(5.0 + shift);

        let angle = sleigh.angle_to(depot.label_center);
        let arrow = PolyLine::must_new(vec![
            sleigh.project_away(Distance::meters(20.0), angle),
            sleigh.project_away(Distance::meters(40.0), angle),
        ])
        .make_arrow(thickness, ArrowCap::Triangle);
        self.draw = ctx.upload(GeomBatch::from(vec![(Color::RED.alpha(0.8), arrow)]));
    }
}

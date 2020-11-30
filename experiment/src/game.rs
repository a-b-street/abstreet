use std::collections::HashMap;

use abstutil::prettyprint_usize;
use geom::{ArrowCap, Circle, Distance, Duration, Line, PolyLine, Polygon, Pt2D, Time};
use kml::ParcelMetadata;
use map_gui::load::MapLoader;
use map_gui::tools::{ColorScale, DivergingScale, SimpleMinimap};
use map_gui::{Cached, SimpleApp, ID};
use map_model::{BuildingID, BuildingType, PathConstraints};
use widgetry::{
    Btn, Color, Drawable, EventCtx, Fill, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    LinearGradient, Outcome, Panel, RewriteColor, State, Text, TextExt, Transition, UpdateType,
    VerticalAlignment, Widget,
};

use crate::animation::{Animator, SnowEffect};
use crate::controls::{Controller, InstantController};
use crate::levels::Config;

const ZOOM: f64 = 10.0;

pub struct Game {
    panel: Panel,
    controls: Box<dyn Controller>,
    minimap: SimpleMinimap,
    animator: Animator,
    snow: SnowEffect,

    sleigh: Pt2D,
    state: SleighState,
    over_bldg: Cached<BuildingID, OverBldg>,
}

impl Game {
    pub fn new(ctx: &mut EventCtx, app: &SimpleApp, config: Config) -> Box<dyn State<SimpleApp>> {
        MapLoader::new(
            ctx,
            app,
            config.map.clone(),
            Box::new(move |ctx, app| {
                ctx.canvas.cam_zoom = ZOOM;

                // TODO We need to use FileLoader for this to work on web. Another good argument
                // for just adding a field to Map and not storing separately.
                // TODO This is also hardcoded to Seattle now, ew!
                let parcels: ParcelMetadata = abstutil::read_binary(
                    abstutil::path("system/seattle/parcels.bin"),
                    &mut abstutil::Timer::throwaway(),
                );

                let state = SleighState::new(ctx, app, config, parcels);
                let sleigh = app.map.get_b(state.depot).label_center;
                ctx.canvas.center_on_map_pt(sleigh);

                let panel = Panel::new(Widget::col(vec![
                    Widget::row(vec![
                        Line("15-minute Santa").small_heading().draw(ctx),
                        Btn::close(ctx),
                    ]),
                    // TODO "Families with presents" or "Deliveries"?
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
                    snow: SnowEffect::new(ctx),

                    sleigh,
                    state,
                    over_bldg: Cached::new(),
                };
                game.update_panel(ctx);
                game.minimap
                    .set_zoom(ctx, app, game.state.config.minimap_zoom);
                Transition::Replace(Box::new(game))
            }),
        )
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
                Btn::text_bg2(format!(
                    "Apply upzone ({} available) -- press the U key",
                    upzones_free
                ))
                .build(ctx, "use upzone", Key::U)
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
        self.state.recalc_depots(ctx, app);
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
                if ctx.is_key_down(Key::Space) && self.state.recharge(b, dt) {
                    self.update_panel(ctx);
                    recharging = true;
                    if self.state.depot != b {
                        self.state.depot = b;
                        self.state.recalc_depots(ctx, app);
                    }
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

                let key = OverBldg::key(app, self.sleigh, &self.state);
                let is_depot = key
                    .map(|b| match self.state.houses.get(&b) {
                        Some(BldgState::Depot) => true,
                        _ => false,
                    })
                    .unwrap_or(false);
                self.over_bldg
                    .update(key, |key| OverBldg::value(ctx, app, key, is_depot));
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
        self.snow.event(ctx);
        if self.state.has_energy() {
            self.state.energyless_arrow = None;
        } else {
            if self.state.energyless_arrow.is_none() {
                self.state.energyless_arrow = Some(EnergylessArrow::new(ctx, self.animator.time));
            }
            let depots = self.state.all_depots();
            self.state.energyless_arrow.as_mut().unwrap().update(
                ctx,
                app,
                self.animator.time,
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
        if let Some(draw) = self.over_bldg.value() {
            g.redraw(&draw.0);
        }
        if !self.state.has_energy() {
            g.redraw(&self.state.draw_all_depots);
        }
        g.draw_polygon(
            Color::RED,
            Circle::new(self.sleigh, Distance::meters(5.0)).to_polygon(),
        );
        self.snow.draw(g);
        self.animator.draw(g);
        if let Some(ref arrow) = self.state.energyless_arrow {
            g.redraw(&arrow.draw);
        }
    }
}

struct SleighState {
    config: Config,

    score: usize,
    energy: Duration,
    houses: HashMap<BuildingID, BldgState>,

    depot: BuildingID,
    cost_to_house: HashMap<BuildingID, Duration>,

    upzones_used: usize,
    upzoned_depots: Vec<BuildingID>,

    draw_all_depots: Drawable,
    // This gets covered up by draw_done_houses, instead of an expensive update
    draw_todo_houses: Drawable,
    draw_done_houses: Drawable,
    energyless_arrow: Option<EnergylessArrow>,
}

impl SleighState {
    fn new(
        ctx: &mut EventCtx,
        app: &SimpleApp,
        config: Config,
        parcels: ParcelMetadata,
    ) -> SleighState {
        let mut houses = HashMap::new();
        let mut depot = None;
        for b in app.map.all_buildings() {
            if let BuildingType::Residential(_) = b.bldg_type {
                if let Some(parcel) = parcels.per_bldg.get(&b.orig_id) {
                    // There are some unused commercial buildings around!
                    if parcel.num_housing_units > 0 {
                        houses.insert(b.id, BldgState::Undelivered(parcel.num_housing_units));
                    }
                }
            } else if !b.amenities.is_empty() {
                // TODO Maybe just food?
                houses.insert(b.id, BldgState::Depot);
                if b.orig_id == config.start_depot {
                    depot = Some(b.id);
                }
            }
        }

        let depot = depot.expect(&format!("can't find {}", config.start_depot));
        let energy = config.max_energy;
        let mut s = SleighState {
            config,

            score: 0,
            energy,
            houses,

            depot,
            cost_to_house: HashMap::new(),

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
        batch.append(
            GeomBatch::load_svg(ctx.prerender, "system/assets/tools/star.svg")
                .centered_on(app.map.get_b(self.depot).label_center)
                .color(RewriteColor::ChangeAll(Color::YELLOW)),
        );

        self.cost_to_house = map_model::connectivity::all_costs_from(
            &app.map,
            self.depot,
            Duration::hours(3),
            PathConstraints::Pedestrian,
        );

        let worst_duration = Duration::minutes(15);
        let cost_scale =
            DivergingScale::new(Color::hex("#5D9630"), Color::WHITE, Color::hex("#A32015"))
                .range(0.0, worst_duration.inner_seconds());

        for b in app.map.all_buildings() {
            match self.houses.get(&b.id) {
                Some(BldgState::Undelivered(housing_units)) => {
                    if let Some(cost) = self.cost_to_house.get(&b.id) {
                        let color = cost_scale.eval(cost.inner_seconds()).unwrap();
                        batch.push(color, b.polygon.clone());
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
        if let Some(BldgState::Undelivered(score)) = self.houses.get(&id).cloned() {
            if let Some(cost) = self.cost_to_house.get(&id) {
                self.score += score;
                self.houses.insert(id, BldgState::Done);
                self.energy -= *cost;
                self.recalc_deliveries(ctx, app);
                return Some(score);
            }
        }
        None
    }

    // True if state change
    fn recharge(&mut self, id: BuildingID, dt: Duration) -> bool {
        if let Some(BldgState::Depot) = self.houses.get(&id) {
            self.energy += self.config.recharge_rate * dt;
            self.energy = self.energy.min(self.config.max_energy);
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

    fn value(ctx: &mut EventCtx, app: &SimpleApp, key: BuildingID, is_depot: bool) -> OverBldg {
        let mut batch = GeomBatch::new();
        // We only want to highlight when we're hovering over a depot and could recharge
        if is_depot {
            let b = app.map.get_b(key);
            batch.push(
                Color::YELLOW,
                b.polygon
                    .to_outline(Distance::meters(0.5))
                    .unwrap_or_else(|_| b.polygon.clone()),
            );

            batch.append(
                Text::from(Line("Hold down SPACEBAR to recharge"))
                    .bg(Color::RED)
                    .render_to_batch(ctx.prerender)
                    .scale(0.2)
                    .centered_on(b.label_center.offset(0.0, -30.0)),
            );
        }

        OverBldg(ctx.upload(batch))
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

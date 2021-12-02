use std::collections::HashMap;

use abstutil::{prettyprint_usize, Counter, Timer};
use geom::{ArrowCap, Distance, Duration, Time};
use map_gui::render::DrawOptions;
use map_model::{IntersectionID, MovementID, PathStep, TurnType};
use sim::TripEndpoint;
use widgetry::mapspace::{DummyID, World};
use widgetry::{
    Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, Spinner, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, ShowEverything, Transition};
use crate::sandbox::dashboards::DashTab;

pub struct TrafficSignalDemand {
    panel: Panel,
    all_demand: HashMap<IntersectionID, Demand>,
    hour: Time,
    world: World<DummyID>,
}

impl TrafficSignalDemand {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let all_demand = ctx.loading_screen("predict all demand", |_, timer| {
            Demand::all_demand(app, timer)
        });

        app.primary.current_selection = None;
        assert!(app.primary.suspended_sim.is_none());
        app.primary.suspended_sim = Some(app.primary.clear_sim());

        let hour = Time::START_OF_DAY;
        let mut state = TrafficSignalDemand {
            all_demand,
            hour,
            world: World::unbounded(),
            panel: Panel::new_builder(Widget::col(vec![
                DashTab::TrafficSignals.picker(ctx, app),
                Text::from_all(vec![
                    Line("Press "),
                    Key::LeftArrow.txt(ctx),
                    Line(" and "),
                    Key::RightArrow.txt(ctx),
                    Line(" to adjust the hour"),
                ])
                .into_widget(ctx),
                Widget::row(vec![
                    "Hour:".text_widget(ctx).centered_vert(),
                    Spinner::widget(
                        ctx,
                        "hour",
                        (Duration::ZERO, Duration::hours(24)),
                        Duration::hours(7),
                        Duration::hours(1),
                    ),
                ]),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        };
        state.rebuild_world(ctx, app);
        Box::new(state)
    }

    fn rebuild_world(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut world = World::bounded(app.primary.map.get_bounds());

        let mut draw_all = GeomBatch::new();
        for (i, demand) in &self.all_demand {
            let cnt_per_movement = demand.count(self.hour);
            let total_demand = cnt_per_movement.sum();

            let mut outlines = Vec::new();
            for (movement, cnt) in cnt_per_movement.consume() {
                let percent = (cnt as f64) / (total_demand as f64);
                let arrow = app.primary.map.get_i(*i).movements[&movement]
                    .geom
                    .make_arrow(percent * Distance::meters(3.0), ArrowCap::Triangle);

                let mut draw_hovered = GeomBatch::new();
                if let Ok(p) = arrow.to_outline(Distance::meters(0.1)) {
                    outlines.push(p.clone());
                    draw_hovered.push(Color::WHITE, p);
                }
                draw_all.push(Color::hex("#A3A3A3"), arrow.clone());
                draw_hovered.push(Color::hex("#EE702E"), arrow.clone());

                world
                    .add_unnamed()
                    .hitbox(arrow)
                    .drawn_in_master_batch()
                    .draw_hovered(draw_hovered)
                    .tooltip(Text::from(format!(
                        "{} / {}",
                        prettyprint_usize(cnt),
                        prettyprint_usize(total_demand)
                    )))
                    .build(ctx);
            }
            draw_all.extend(Color::WHITE, outlines);
        }
        world.draw_master_batch(ctx, draw_all);

        world.initialize_hover(ctx);
        self.world = world;
    }
}

impl State<App> for TrafficSignalDemand {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.world.event(ctx);

        let mut changed = false;
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    app.primary.sim = app.primary.suspended_sim.take().unwrap();
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                if let Some(tab) = DashTab::TrafficSignals.tab_changed(app, &self.panel) {
                    app.primary.sim = app.primary.suspended_sim.take().unwrap();
                    return Transition::Replace(tab.launch(ctx, app));
                }
                changed = true;
            }
            _ => {}
        }
        if ctx.input.pressed(Key::LeftArrow) {
            self.panel
                .modify_spinner(ctx, "hour", -1.0 * Duration::hours(1));
            changed = true;
        }
        if ctx.input.pressed(Key::RightArrow) {
            self.panel.modify_spinner(ctx, "hour", Duration::hours(1));
            changed = true;
        }
        if changed {
            self.hour = Time::START_OF_DAY + self.panel.spinner("hour");
            self.rebuild_world(ctx, app);
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.suppress_traffic_signal_details
            .extend(self.all_demand.keys().cloned());
        app.draw(g, opts, &ShowEverything::new());

        self.panel.draw(g);
        self.world.draw(g);
    }
}

struct Demand {
    // Unsorted
    raw: Vec<(Time, MovementID)>,
}

impl Demand {
    fn all_demand(app: &App, timer: &mut Timer) -> HashMap<IntersectionID, Demand> {
        let map = &app.primary.map;

        let mut all_demand = HashMap::new();
        for i in map.all_intersections() {
            if i.is_traffic_signal() {
                all_demand.insert(i.id, Demand { raw: Vec::new() });
            }
        }

        let paths = timer
            .parallelize(
                "predict routes",
                app.primary.sim.all_trip_info(),
                |(_, trip)| {
                    let departure = trip.departure;
                    TripEndpoint::path_req(trip.start, trip.end, trip.mode, map)
                        .and_then(|req| map.pathfind(req).ok())
                        .map(|path| (departure, path))
                },
            )
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        timer.start_iter("compute demand", paths.len());
        for (now, path) in paths {
            timer.next();
            // TODO For every step, increase 'now' by the best-case time to cross that step.
            for step in path.get_steps() {
                match step {
                    PathStep::Lane(_) | PathStep::ContraflowLane(_) => {}
                    PathStep::Turn(t) | PathStep::ContraflowTurn(t) => {
                        if map.get_t(*t).turn_type == TurnType::SharedSidewalkCorner {
                            continue;
                        }
                        if let Some(demand) = all_demand.get_mut(&t.parent) {
                            demand
                                .raw
                                .push((now, map.get_i(t.parent).turn_to_movement(*t).0));
                        }
                    }
                }
            }
        }

        all_demand
    }

    fn count(&self, start: Time) -> Counter<MovementID> {
        let end = start + Duration::hours(1);
        let mut cnt = Counter::new();
        for (t, m) in &self.raw {
            if *t >= start && *t <= end {
                cnt.inc(*m);
            }
        }
        cnt
    }
}

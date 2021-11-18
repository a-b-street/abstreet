use std::collections::BTreeSet;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;

use abstutil::Timer;
use geom::{Distance, Duration, Polygon, Time};
use map_gui::ID;
use map_model::IntersectionID;
use sim::{IndividTrip, PersonSpec, Scenario, TripEndpoint, TripMode, TripPurpose};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State,
    TextExt, UpdateType, VerticalAlignment, Widget,
};

use super::Neighborhood;
use crate::app::{App, Transition};
use crate::sandbox::TimePanel;

// A very simplified SandboxMode
pub struct TrafficSim {
    panel: Panel,
    time_panel: TimePanel,
    neighborhood: Neighborhood,
    draw_study_area: Drawable,
}

impl TrafficSim {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        neighborhood: Neighborhood,
    ) -> Box<dyn State<App>> {
        let borders = grow_simulation_area(app, &neighborhood);
        let draw_study_area = draw_study_area(app, &borders).upload(ctx);
        spawn_traffic(app, &neighborhood, borders.into_iter().collect());

        Box::new(TrafficSim {
            panel: Panel::new_builder(Widget::col(vec![
                "Simulating perimeter traffic".text_widget(ctx),
                ctx.style()
                    .btn_outline
                    .text("back")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            time_panel: TimePanel::new(ctx, app),
            neighborhood,
            draw_study_area,
        })
    }
}

impl State<App> for TrafficSim {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "back" => {
                    app.primary.clear_sim();
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<TrafficSim>().ok().unwrap();
                        vec![super::viewer::Viewer::new_state(
                            ctx,
                            app,
                            state.neighborhood,
                        )]
                    }));
                }
                _ => unreachable!(),
            }
        }

        // TODO Ideally here reset to midnight would jump back to when the preview started?
        if let Some(t) = self.time_panel.event(ctx, app, None) {
            return t;
        }
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }
        // Only hover on cars
        if !matches!(app.primary.current_selection, Some(ID::Car(_))) {
            app.primary.current_selection = None;
        }

        if self.time_panel.is_paused() {
            Transition::Keep
        } else {
            ctx.request_update(UpdateType::Game);
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.time_panel.draw(g);

        g.redraw(&self.neighborhood.fade_irrelevant);
        g.redraw(&self.neighborhood.draw_filters);
        g.redraw(&self.draw_study_area);
    }
}

// We start with just an area bounded by major perimeter roads. Expand out one set of roads from
// that perimeter and find all of the intersections we reach. Use that as border intersections for
// the area to simulate.
//
// TODO Maybe let people adjust this. Probably takes local knowledge to know interesting
// sources/sinks for traffic near the neighborhood.
fn grow_simulation_area(app: &App, neighborhood: &Neighborhood) -> BTreeSet<IntersectionID> {
    let map = &app.primary.map;
    let mut borders = BTreeSet::new();
    for i in &neighborhood.borders {
        for r in &map.get_i(*i).roads {
            if neighborhood.orig_perimeter.interior.contains(r) {
                continue;
            }
            let road = map.get_r(*r);
            for i in [road.src_i, road.dst_i] {
                if !neighborhood.borders.contains(&i) {
                    borders.insert(i);
                }
            }
        }
    }
    borders
}

fn draw_study_area(app: &App, borders: &BTreeSet<IntersectionID>) -> GeomBatch {
    let mut batch = GeomBatch::new();
    let area = Polygon::convex_hull(
        borders
            .iter()
            .map(|i| app.primary.map.get_i(*i).polygon.clone())
            .collect(),
    );
    if let Ok(p) = area.to_outline(Distance::meters(2.0)) {
        batch.push(Color::RED.alpha(0.8), p);
    }
    batch
}

fn spawn_traffic(app: &mut App, neighborhood: &Neighborhood, borders: Vec<IntersectionID>) {
    // Create through-traffic -- a bunch of cars going between pairs of borders. Will they try to
    // route through the LTN? Maybe force some percentage of them to... user configurable, or make
    // them route around traffic delays?
    let map = &app.primary.map;
    let mut rng = app.primary.current_flags.sim_flags.make_rng();
    let mut scenario = Scenario::empty(map, "perimeter traffic");

    let num_cars = 500;
    let uniformly_spawn = Duration::seconds(10.0);

    for _ in 0..num_cars {
        let from = *borders.choose(&mut rng).unwrap();
        // TODO Pick one kind of far away
        let to = *borders.choose(&mut rng).unwrap();
        if from == to {
            continue;
        }
        // TODO Restrict to paths passing through the study area
        scenario.people.push(PersonSpec {
            orig_id: None,
            trips: vec![IndividTrip::new(
                Time::START_OF_DAY + rand_duration(&mut rng, Duration::ZERO, uniformly_spawn),
                TripPurpose::Work,
                TripEndpoint::Border(from),
                TripEndpoint::Border(to),
                TripMode::Drive,
            )],
        });
    }

    let retry_if_no_room = true;
    scenario.instantiate_without_retries(
        &mut app.primary.sim,
        map,
        &mut rng,
        retry_if_no_room,
        &mut Timer::throwaway(),
    );
    app.primary.sim.tiny_step(map, &mut app.primary.sim_cb);
}

fn rand_duration(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    assert!(high > low);
    Duration::seconds(rng.gen_range(low.inner_seconds()..high.inner_seconds()))
}

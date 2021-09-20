use abstutil::Timer;
use geom::{Distance, Duration};
use map_gui::load::FileLoader;
use map_model::Position;
use sim::{Scenario, TripEndpoint, TripMode};
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Outcome, Panel, State, TextExt, VerticalAlignment,
    Widget,
};

use crate::app::{App, Transition};
use crate::ungap::{Layers, Tab, TakeLayers};

pub struct ShowGaps {
    top_panel: Panel,
    layers: Layers,
}

impl TakeLayers for ShowGaps {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

impl ShowGaps {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        let map_name = app.primary.map.get_name().clone();
        if app.session.mode_shift.key().as_ref() == Some(&map_name) {
            return Box::new(ShowGaps {
                top_panel: make_top_panel(ctx, app),
                layers,
            });
        }

        let scenario_name = crate::pregame::default_scenario_for_map(&map_name);
        if scenario_name == "home_to_work" {
            // TODO Should we generate and use this scenario? Or maybe just disable this mode
            // entirely?
            app.session.mode_shift.set(
                map_name,
                ModeShiftData {
                    all_candidate_trips: Vec::new(),
                    filters: Filters::default(),
                },
            );
            ShowGaps::new_state(ctx, app, layers)
        } else {
            FileLoader::<App, Scenario>::new_state(
                ctx,
                abstio::path_scenario(&map_name, &scenario_name),
                Box::new(|ctx, app, _, maybe_scenario| {
                    // TODO Handle corrupt files
                    let scenario = maybe_scenario.unwrap();
                    let data = ctx.loading_screen("predict mode shift", |_, timer| {
                        ModeShiftData::from_scenario(app, scenario, timer)
                    });
                    app.session.mode_shift.set(map_name, data);
                    Transition::Replace(ShowGaps::new_state(ctx, app, layers))
                }),
            )
        }
    }
}

impl State<App> for ShowGaps {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => {
                return Tab::PredictImpact
                    .handle_action::<ShowGaps>(ctx, app, &x)
                    .unwrap();
            }
            _ => {}
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_panel.draw(g);
    }
}

fn make_top_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    let data = app.session.mode_shift.value().unwrap();
    let col = vec![
        Tab::PredictImpact.make_header(ctx, app),
        // TODO Info button with popup explaining all the assumptions... (where scenario data comes
        // from, only driving -> cycling, no off-map starts or ends, etc)
        abstutil::prettyprint_usize(data.all_candidate_trips.len()).text_widget(ctx),
    ];

    Panel::new_builder(Widget::col(col))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx)
}

// TODO For now, it's easier to just copy pieces from sandbox/dashboards/mode_shift.rs. I'm not
// sure how these two tools will interact yet, so not worth trying to refactor anything. One works
// off Scenario files directly, the other off an instantiated Scenario.

pub struct ModeShiftData {
    // Calculated from the unedited map, not yet filtered.
    all_candidate_trips: Vec<CandidateTrip>,
    filters: Filters,
    // TODO Network gaps, counts (or total trip distances?) per road. Relative to the unedited map,
    // but with filters applied.
    // TODO Then a score, comparing those gaps with the current map edits.
}

struct Filters {
    max_driving_time: Duration,
    max_biking_time: Duration,
    max_biking_distance: Distance,
    max_elevation_gain: Distance,
}

impl Filters {
    fn default() -> Filters {
        Filters {
            max_driving_time: Duration::minutes(30),
            max_biking_time: Duration::minutes(30),
            max_biking_distance: Distance::miles(10.0),
            max_elevation_gain: Distance::feet(30.0),
        }
    }
}

struct CandidateTrip {
    bike_from: Position,
    bike_to: Position,

    estimated_driving_time: Duration,
    estimated_biking_time: Duration,
    biking_distance: Distance,
    total_elevation_gain: Distance,
    total_elevation_loss: Distance,
}

impl ModeShiftData {
    fn from_scenario(app: &App, scenario: Scenario, timer: &mut Timer) -> ModeShiftData {
        let map = app
            .primary
            .unedited_map
            .as_ref()
            .unwrap_or(&app.primary.map);
        let all_candidate_trips = timer
            .parallelize(
                "analyze trips",
                scenario
                    .all_trips()
                    .filter(|trip| {
                        trip.mode == TripMode::Drive
                            && matches!(trip.origin, TripEndpoint::Bldg(_))
                            && matches!(trip.destination, TripEndpoint::Bldg(_))
                    })
                    .collect(),
                |trip| {
                    // TODO Does ? work
                    if let (Some(driving_path), Some(biking_path)) = (
                        TripEndpoint::path_req(trip.origin, trip.destination, TripMode::Drive, map)
                            .and_then(|req| map.pathfind(req).ok()),
                        TripEndpoint::path_req(trip.origin, trip.destination, TripMode::Bike, map)
                            .and_then(|req| map.pathfind(req).ok()),
                    ) {
                        let (total_elevation_gain, total_elevation_loss) =
                            biking_path.get_total_elevation_change(map);
                        Some(CandidateTrip {
                            bike_from: biking_path.get_req().start,
                            bike_to: biking_path.get_req().end,

                            estimated_driving_time: driving_path.estimate_duration(map, None),
                            estimated_biking_time: biking_path
                                .estimate_duration(map, Some(map_model::MAX_BIKE_SPEED)),
                            biking_distance: biking_path.total_length(),
                            total_elevation_gain,
                            total_elevation_loss,
                        })
                    } else {
                        None
                    }
                },
            )
            .into_iter()
            .flatten()
            .collect();
        ModeShiftData {
            filters: Filters::default(),
            all_candidate_trips,
        }
    }
}

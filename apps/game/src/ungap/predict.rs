use std::collections::HashSet;

use abstio::Manifest;
use abstutil::{prettyprint_bytes, prettyprint_usize, Counter, Timer};
use geom::{Distance, Duration, UnitFmt};
use map_gui::tools::{percentage_bar, ColorNetwork};
use map_gui::ID;
use map_model::{PathRequest, PathStepV2, RoadID};
use synthpop::{Scenario, TripEndpoint, TripMode};
use widgetry::mapspace::ToggleZoomed;
use widgetry::tools::{open_browser, FileLoader};
use widgetry::{EventCtx, GfxCtx, Line, Outcome, Panel, Spinner, State, Text, TextExt, Widget};

use crate::app::{App, Transition};
use crate::ungap::{Layers, Tab, TakeLayers};

pub struct ShowGaps {
    top_panel: Panel,
    layers: Layers,
    tooltip: Option<Text>,
}

impl TakeLayers for ShowGaps {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

impl ShowGaps {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        Box::new(ShowGaps {
            top_panel: make_top_panel(ctx, app),
            layers,
            tooltip: None,
        })
    }
}

impl State<App> for ShowGaps {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            self.tooltip = None;
            if let Some(data) = app.session.mode_shift.value() {
                if let Some(r) = match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    Some(ID::Road(r)) => Some(r),
                    Some(ID::Lane(l)) => Some(l.road),
                    _ => None,
                } {
                    let count = data.gaps.count_per_road.get(r);
                    if count > 0 {
                        // TODO Word more precisely... or less verbosely.
                        self.tooltip = Some(Text::from(Line(format!(
                            "{} trips might cross this high-stress road",
                            prettyprint_usize(count)
                        ))));
                    }
                }
            }
        }

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "read about how this prediction works" {
                    open_browser("https://a-b-street.github.io/docs/software/ungap_the_map/tech_details.html#predict-impact");
                    return Transition::Keep;
                } else if x == "Calculate" {
                    let change_key = app.primary.map.get_edits_change_key();
                    let map_name = app.primary.map.get_name().clone();
                    let scenario_name = Scenario::default_scenario_for_map(&map_name);
                    return Transition::Push(FileLoader::<App, Scenario>::new_state(
                        ctx,
                        abstio::path_scenario(&map_name, &scenario_name),
                        Box::new(move |ctx, app, timer, maybe_scenario| {
                            // TODO Handle corrupt files
                            let scenario = maybe_scenario.unwrap();
                            let data = ModeShiftData::from_scenario(ctx, app, scenario, timer);
                            app.session.mode_shift.set((map_name, change_key), data);

                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::ConsumeState(Box::new(|state, ctx, app| {
                                    let state = state.downcast::<ShowGaps>().ok().unwrap();
                                    vec![ShowGaps::new_state(ctx, app, state.take_layers())]
                                })),
                            ])
                        }),
                    ));
                }

                return Tab::PredictImpact
                    .handle_action::<ShowGaps>(ctx, app, &x)
                    .unwrap();
            }
            Outcome::Changed(_) => {
                let (map_name, mut data) = app.session.mode_shift.take().unwrap();
                data.filters = Filters::from_controls(&self.top_panel);
                ctx.loading_screen("update mode shift", |ctx, timer| {
                    data.recalculate_gaps(ctx, app, timer)
                });
                app.session.mode_shift.set(map_name, data);
                // TODO This is heavy-handed for just updating the counters
                self.top_panel = make_top_panel(ctx, app);
            }
            _ => {}
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.layers.draw(g, app);

        if let Some(data) = app.session.mode_shift.value() {
            data.gaps.draw.draw(g);
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
}

fn make_top_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    let map_name = app.primary.map.get_name().clone();
    let change_key = app.primary.map.get_edits_change_key();
    let col;

    if app.session.mode_shift.key().as_ref() == Some(&(map_name.clone(), change_key)) {
        let data = app.session.mode_shift.value().unwrap();

        col = vec![
            ctx.style()
                .btn_plain
                .icon_text(
                    "system/assets/tools/info.svg",
                    "How many drivers might switch to biking?",
                )
                .build_widget(ctx, "read about how this prediction works"),
            percentage_bar(
                ctx,
                Text::from(Line(format!(
                    "{} total driving trips in this area",
                    prettyprint_usize(data.all_candidate_trips.len())
                ))),
                0.0,
            ),
            Widget::col(vec![
                "Who might cycle if it was safer?".text_widget(ctx),
                data.filters.to_controls(ctx),
                percentage_bar(
                    ctx,
                    Text::from(Line(format!(
                        "{} / {} trips, based on these thresholds",
                        prettyprint_usize(data.filtered_trips.len()),
                        prettyprint_usize(data.all_candidate_trips.len())
                    ))),
                    pct(data.filtered_trips.len(), data.all_candidate_trips.len()),
                ),
            ])
            .section(ctx),
            Widget::col(vec![
                "How many would switch based on your proposal?".text_widget(ctx),
                percentage_bar(
                    ctx,
                    Text::from(Line(format!(
                        "{} / {} trips would switch",
                        prettyprint_usize(data.results.num_trips),
                        prettyprint_usize(data.all_candidate_trips.len())
                    ))),
                    pct(data.results.num_trips, data.all_candidate_trips.len()),
                ),
                data.results.describe().into_widget(ctx),
            ])
            .section(ctx),
        ];
    } else {
        let scenario_name = Scenario::default_scenario_for_map(&map_name);
        if scenario_name == "home_to_work" {
            col =
                vec!["This city doesn't have travel demand model data available".text_widget(ctx)];
        } else {
            let size = Manifest::load()
                .get_entry(&abstio::path_scenario(&map_name, &scenario_name))
                .map(|entry| prettyprint_bytes(entry.compressed_size_bytes))
                .unwrap_or_else(|| "???".to_string());
            col = vec![
                Text::from_multiline(vec![
                    Line("Predicting impact of your proposal may take a moment."),
                    Line("The application may freeze up during that time."),
                    Line(format!("We need to load a {} file", size)),
                ])
                .into_widget(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text("Calculate")
                    .build_def(ctx),
            ];
        }
    }

    Tab::PredictImpact.make_left_panel(ctx, app, Widget::col(col))
}

// TODO For now, it's easier to just copy pieces from sandbox/dashboards/mode_shift.rs. I'm not
// sure how these two tools will interact yet, so not worth trying to refactor anything. One works
// off Scenario files directly, the other off an instantiated Scenario.

pub struct ModeShiftData {
    // Calculated from the unedited map, not yet filtered.
    all_candidate_trips: Vec<CandidateTrip>,
    filters: Filters,
    // From the unedited map, filtered
    gaps: NetworkGaps,
    // Indices into all_candidate_trips
    filtered_trips: Vec<usize>,
    results: Results,
}

struct CandidateTrip {
    bike_req: PathRequest,
    estimated_biking_time: Duration,
    driving_distance: Distance,
    total_elevation_gain: Distance,
}

struct Filters {
    max_biking_time: Duration,
    max_elevation_gain: Distance,
}

struct NetworkGaps {
    draw: ToggleZoomed,
    count_per_road: Counter<RoadID>,
}

// Of the filtered trips, which cross at least 1 edited road?
// TODO Many ways of defining this... maybe the edits need to plug the gap on at least 50% of
// stressful roads encountered by this trip?
struct Results {
    num_trips: usize,
    total_driving_distance: Distance,
    annual_co2_emissions_tons: f64,
}

impl Filters {
    fn default() -> Self {
        Self {
            max_biking_time: Duration::minutes(30),
            max_elevation_gain: Distance::feet(100.0),
        }
    }

    fn apply(&self, x: &CandidateTrip) -> bool {
        x.estimated_biking_time <= self.max_biking_time
            && x.total_elevation_gain <= self.max_elevation_gain
    }

    fn to_controls(&self, ctx: &mut EventCtx) -> Widget {
        Widget::col(vec![
            Widget::row(vec![
                "Max biking time".text_widget(ctx).centered_vert(),
                Spinner::widget(
                    ctx,
                    "max_biking_time",
                    (Duration::ZERO, Duration::hours(12)),
                    self.max_biking_time,
                    Duration::minutes(1),
                ),
            ]),
            Widget::row(vec![
                "Max elevation gain".text_widget(ctx).centered_vert(),
                Spinner::widget_with_custom_rendering(
                    ctx,
                    "max_elevation_gain",
                    (Distance::ZERO, Distance::feet(500.0)),
                    self.max_elevation_gain,
                    Distance::feet(10.0),
                    // Even if the user's settings are set to meters, our step size is in feet, so
                    // just render in feet.
                    Box::new(|x| {
                        x.to_string(&UnitFmt {
                            round_durations: false,
                            metric: false,
                        })
                    }),
                ),
            ]),
        ])
    }

    fn from_controls(panel: &Panel) -> Filters {
        Filters {
            max_biking_time: panel.spinner("max_biking_time"),
            max_elevation_gain: panel.spinner("max_elevation_gain"),
        }
    }
}

impl Results {
    fn default() -> Self {
        Self {
            num_trips: 0,
            total_driving_distance: Distance::ZERO,
            annual_co2_emissions_tons: 0.0,
        }
    }

    fn describe(&self) -> Text {
        let mut txt = Text::new();
        txt.add_line(Line(format!(
            "{} total vehicle miles traveled daily, now eliminated",
            prettyprint_usize(self.total_driving_distance.to_miles() as usize)
        )));
        // Round to 1 decimal place
        let tons = (self.annual_co2_emissions_tons * 10.0).round() / 10.0;
        txt.add_line(Line(format!(
            "{} tons of CO2 emissions saved annually",
            tons
        )));
        txt
    }
}

impl ModeShiftData {
    fn empty(ctx: &mut EventCtx) -> Self {
        Self {
            all_candidate_trips: Vec::new(),
            filters: Filters::default(),
            gaps: NetworkGaps {
                draw: ToggleZoomed::empty(ctx),
                count_per_road: Counter::new(),
            },
            filtered_trips: Vec::new(),
            results: Results::default(),
        }
    }

    fn from_scenario(
        ctx: &mut EventCtx,
        app: &App,
        scenario: Scenario,
        timer: &mut Timer,
    ) -> ModeShiftData {
        let unedited_map = app
            .secondary
            .as_ref()
            .map(|x| &x.map)
            .unwrap_or(&app.primary.map);
        let all_candidate_trips = timer
            .parallelize(
                "analyze trips",
                scenario
                    .all_trips()
                    .filter(|trip| {
                        trip.mode == TripMode::Drive
                            && matches!(trip.origin, TripEndpoint::Building(_))
                            && matches!(trip.destination, TripEndpoint::Building(_))
                    })
                    .collect(),
                |trip| {
                    // TODO Does ? work
                    if let (Some(driving_path), Some(biking_path)) = (
                        TripEndpoint::path_req(
                            trip.origin,
                            trip.destination,
                            TripMode::Drive,
                            unedited_map,
                        )
                        .and_then(|req| unedited_map.pathfind(req).ok()),
                        TripEndpoint::path_req(
                            trip.origin,
                            trip.destination,
                            TripMode::Bike,
                            unedited_map,
                        )
                        .and_then(|req| unedited_map.pathfind(req).ok()),
                    ) {
                        let (total_elevation_gain, _) =
                            biking_path.get_total_elevation_change(unedited_map);
                        Some(CandidateTrip {
                            bike_req: biking_path.get_req().clone(),
                            estimated_biking_time: biking_path
                                .estimate_duration(unedited_map, Some(map_model::MAX_BIKE_SPEED)),
                            driving_distance: driving_path.total_length(),
                            total_elevation_gain,
                        })
                    } else {
                        None
                    }
                },
            )
            .into_iter()
            .flatten()
            .collect();
        let mut data = ModeShiftData::empty(ctx);
        data.all_candidate_trips = all_candidate_trips;
        data.recalculate_gaps(ctx, app, timer);
        data
    }

    fn recalculate_gaps(&mut self, ctx: &mut EventCtx, app: &App, timer: &mut Timer) {
        let unedited_map = app
            .secondary
            .as_ref()
            .map(|x| &x.map)
            .unwrap_or(&app.primary.map);

        // Find all high-stress roads, since we'll filter by them next
        let mut high_stress = HashSet::new();
        for r in unedited_map.all_roads() {
            for dr in r.id.both_directions() {
                if r.high_stress_for_bikes(unedited_map, dr.dir) {
                    high_stress.insert(dr);
                }
            }
        }

        self.filtered_trips.clear();
        let mut filtered_requests = Vec::new();
        for (idx, trip) in self.all_candidate_trips.iter().enumerate() {
            if self.filters.apply(trip) {
                self.filtered_trips.push(idx);
                filtered_requests.push((idx, trip.bike_req.clone()));
            }
        }

        self.results = Results::default();

        let mut count_per_road = Counter::new();
        for (idx, path) in timer
            .parallelize("calculate routes", filtered_requests, |(idx, req)| {
                unedited_map.pathfind_v2(req).map(|path| (idx, path))
            })
            .into_iter()
            .flatten()
        {
            let mut crosses_edited_road = false;
            for step in path.get_steps() {
                // No Contraflow steps for bike paths
                if let PathStepV2::Along(dr) = step {
                    if high_stress.contains(dr) {
                        count_per_road.inc(dr.road);

                        // TODO Assumes the edits have made the road stop being high stress!
                        if !crosses_edited_road
                            && app.primary.map.get_edits().changed_roads.contains(&dr.road)
                        {
                            crosses_edited_road = true;
                        }
                    }
                }
            }
            if crosses_edited_road {
                self.results.num_trips += 1;
                self.results.total_driving_distance +=
                    self.all_candidate_trips[idx].driving_distance;
            }
        }

        // Assume this trip happens 5 times a week, 52 weeks a year.
        let annual_mileage = 5.0 * 52.0 * self.results.total_driving_distance.to_miles();
        // https://www.epa.gov/greenvehicles/greenhouse-gas-emissions-typical-passenger-vehicle#driving
        // says 404 grams per mile.
        // And convert grams to tons
        self.results.annual_co2_emissions_tons = 404.0 * annual_mileage / 907185.0;

        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(count_per_road.clone(), &app.cs.good_to_bad_red);
        self.gaps = NetworkGaps {
            draw: colorer.build(ctx),
            count_per_road,
        };
    }
}

fn pct(value: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        value as f64 / total as f64
    }
}

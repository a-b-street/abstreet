use abstio::MapName;
use abstutil::{Counter, Timer};
use map_gui::load::FileLoader;
use map_gui::tools::ColorNetwork;
use map_model::{PathRequest, PathStepV2, RoadID};
use sim::{Scenario, TripEndpoint, TripMode};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Panel, SimpleState, State, TextExt, VerticalAlignment,
    Widget,
};

use crate::{App, Transition};

// TODO Tooltips
// TODO Intersections
// TODO Toggle before/after / compare directly
// TODO Share structure or pieces with Ungap's predict mode

pub struct Results {
    map: MapName,
    all_driving_trips: Vec<PathRequest>,

    // TODO Or a World with tooltips baked in... except there's less flexibility to toggle views
    // dynamically
    before_draw_heatmap: ToggleZoomed,
    before_counts: Counter<RoadID>,
    after_draw_heatmap: ToggleZoomed,
    after_counts: Counter<RoadID>,
}

impl Results {
    fn from_scenario(
        ctx: &mut EventCtx,
        app: &App,
        scenario: Scenario,
        timer: &mut Timer,
    ) -> Results {
        let map = &app.map;
        let all_driving_trips = timer
            .parallelize(
                "analyze trips",
                scenario
                    .all_trips()
                    .filter(|trip| trip.mode == TripMode::Drive)
                    .collect(),
                |trip| TripEndpoint::path_req(trip.origin, trip.destination, TripMode::Drive, map),
            )
            .into_iter()
            .flatten()
            .collect();

        let mut results = Results {
            map: app.map.get_name().clone(),
            all_driving_trips,

            before_draw_heatmap: ToggleZoomed::empty(ctx),
            before_counts: Counter::new(),
            after_draw_heatmap: ToggleZoomed::empty(ctx),
            after_counts: Counter::new(),
        };
        results.recalculate_impact(ctx, app, timer);
        results
    }

    fn recalculate_impact(&mut self, ctx: &mut EventCtx, app: &App, timer: &mut Timer) {
        self.before_counts = Counter::new();
        self.after_counts = Counter::new();

        let map = &app.map;
        for path in timer
            .parallelize("calculate routes", self.all_driving_trips.clone(), |req| {
                map.pathfind_v2(req)
            })
            .into_iter()
            .flatten()
        {
            for step in path.get_steps() {
                // No Contraflow steps for driving paths
                if let PathStepV2::Along(dr) = step {
                    self.before_counts.inc(dr.road);
                }
            }
        }

        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(self.before_counts.clone(), &app.cs.good_to_bad_red);
        self.before_draw_heatmap = colorer.build(ctx);
    }
}

pub struct ShowResults;

impl ShowResults {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let map_name = app.map.get_name().clone();
        // TODO Handle changes in the filters / partitioning too
        if app
            .session
            .impact
            .as_ref()
            .map(|i| i.map != map_name)
            .unwrap_or(true)
        {
            let scenario_name = Scenario::default_scenario_for_map(&map_name);
            return FileLoader::<App, Scenario>::new_state(
                ctx,
                abstio::path_scenario(&map_name, &scenario_name),
                Box::new(move |ctx, app, timer, maybe_scenario| {
                    // TODO Handle corrupt files
                    let scenario = maybe_scenario.unwrap();
                    app.session.impact = Some(Results::from_scenario(ctx, app, scenario, timer));
                    Transition::Replace(ShowResults::new_state(ctx, app))
                }),
            );
        }

        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Impact prediction".text_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(ShowResults))
    }
}

impl SimpleState<App> for ShowResults {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        if x == "close" {
            return Transition::Pop;
        }
        unreachable!()
    }

    // TODO Or on_mouseover?
    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let impact = app.session.impact.as_ref().unwrap();
        impact.before_draw_heatmap.draw(g);
    }
}

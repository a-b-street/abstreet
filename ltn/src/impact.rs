use abstio::MapName;
use abstutil::{prettyprint_usize, Counter, Timer};
use map_gui::load::FileLoader;
use map_gui::tools::{cmp_count, ColorNetwork, DivergingScale};
use map_gui::ID;
use map_model::{PathRequest, PathStepV2, RoadID};
use sim::{Scenario, TripEndpoint, TripMode};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState, State, Text,
    TextExt, VerticalAlignment, Widget,
};

use crate::{App, Transition};

// TODO Intersections
// TODO Configurable main road penalty, like in the pathfinding tool
// TODO Don't allow crossing filters at all -- don't just disincentivize
// TODO Share structure or pieces with Ungap's predict mode
// ... can't we just produce data of a certain shape, and have a UI pretty tuned for that?

pub struct Results {
    map: MapName,
    all_driving_trips: Vec<PathRequest>,

    // TODO Or a World with tooltips baked in... except there's less flexibility to toggle views
    // dynamically
    before_draw_heatmap: ToggleZoomed,
    before_counts: Counter<RoadID>,
    after_draw_heatmap: ToggleZoomed,
    after_counts: Counter<RoadID>,
    relative_draw_heatmap: ToggleZoomed,
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
            relative_draw_heatmap: ToggleZoomed::empty(ctx),
        };
        results.recalculate_impact(ctx, app, timer);
        results
    }

    fn recalculate_impact(&mut self, ctx: &mut EventCtx, app: &App, timer: &mut Timer) {
        self.before_counts = Counter::new();
        self.after_counts = Counter::new();
        let map = &app.map;

        // Before the filters
        for path in timer
            .parallelize(
                "calculate routes before filters",
                self.all_driving_trips.clone(),
                |req| map.pathfind_v2(req),
            )
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

        // After the filters
        let mut params = map.routing_params().clone();
        app.session.modal_filters.update_routing_params(&mut params);
        let cache_custom = true;
        for path in timer
            .parallelize(
                "calculate routes after filters",
                self.all_driving_trips.clone(),
                |req| map.pathfind_v2_with_params(req, &params, cache_custom),
            )
            .into_iter()
            .flatten()
        {
            for step in path.get_steps() {
                // No Contraflow steps for driving paths
                if let PathStepV2::Along(dr) = step {
                    self.after_counts.inc(dr.road);
                }
            }
        }
        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(self.after_counts.clone(), &app.cs.good_to_bad_red);
        self.after_draw_heatmap = colorer.build(ctx);

        // Relative diff
        let mut colorer = ColorNetwork::no_fading(app);
        // TODO I really need help understanding how to do this. If the average isn't 1.0 (meaning
        // no change), then the colors are super wacky.
        let scale = DivergingScale::new(Color::hex("#5D9630"), Color::WHITE, Color::hex("#A32015"))
            .range(0.0, 2.0);

        let mut min_ratio: f64 = 100000.0;
        let mut max_ratio: f64 = 0.0;

        for (r, before, after) in self
            .before_counts
            .clone()
            .compare(self.after_counts.clone())
        {
            let ratio = (after as f64) / (before as f64);
            if let Some(c) = scale.eval(ratio) {
                colorer.add_r(r, c);
            }
            min_ratio = min_ratio.min(ratio);
            max_ratio = max_ratio.max(ratio);
        }
        info!("The ratios were between {min_ratio:.2} and {max_ratio:.2}");
        self.relative_draw_heatmap = colorer.build(ctx);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Layer {
    Before,
    After,
    Relative,
}

pub struct ShowResults {
    layer: Layer,
    tooltip: Option<Text>,
}

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

        let layer = Layer::Relative;
        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Impact prediction".text_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            "This shows how many driving trips cross each road".text_widget(ctx),
            Widget::row(vec![
                "Show what?".text_widget(ctx).centered_vert(),
                Widget::dropdown(
                    ctx,
                    "layer",
                    layer,
                    vec![
                        Choice::new("before", Layer::Before),
                        Choice::new("after", Layer::After),
                        Choice::new("relative", Layer::Relative),
                    ],
                ),
            ]),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(ShowResults {
                layer,
                tooltip: None,
            }),
        )
    }
}

impl SimpleState<App> for ShowResults {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        if x == "close" {
            return Transition::Pop;
        }
        unreachable!()
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, app: &mut App) {
        self.tooltip = None;
        if let Some(r) = match app.mouseover_unzoomed_roads_and_intersections(ctx) {
            Some(ID::Road(r)) => Some(r),
            Some(ID::Lane(l)) => Some(l.road),
            _ => None,
        } {
            let impact = app.session.impact.as_ref().unwrap();
            let before = impact.before_counts.get(r);
            let after = impact.after_counts.get(r);
            let mut txt = Text::from_multiline(vec![
                Line(format!("Before: {}", prettyprint_usize(before))),
                Line(format!("After: {}", prettyprint_usize(after))),
            ]);
            cmp_count(&mut txt, before, after);
            txt.add_line(Line(format!(
                "After/before: {:.2}",
                (after as f64) / (before as f64)
            )));
            self.tooltip = Some(txt);
        }
    }

    fn panel_changed(
        &mut self,
        _: &mut EventCtx,
        _: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition> {
        self.layer = panel.dropdown_value("layer");
        None
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let impact = app.session.impact.as_ref().unwrap();
        match self.layer {
            Layer::Before => {
                impact.before_draw_heatmap.draw(g);
            }
            Layer::After => {
                impact.after_draw_heatmap.draw(g);
            }
            Layer::Relative => {
                impact.relative_draw_heatmap.draw(g);
            }
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
}

use abstutil::{prettyprint_usize, Counter, Parallelism, Timer};
use map_gui::colors::ColorSchemeChoice;
use map_gui::tools::ColorNetwork;
use map_gui::{AppLike, ID};
use map_model::{PathRequest, RoadID, RoutingParams, Traversable, NORMAL_LANE_THICKNESS};
use sim::{TripEndpoint, TripMode};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    Spinner, State, Text, TextExt, TextSpan, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::CommonState;

/// See how live-tuned routing parameters affect a single request.
pub struct RouteExplorer {
    panel: Panel,
    start: TripEndpoint,
    // (endpoint, confirmed, render the paths to it)
    goal: Option<(TripEndpoint, bool, Drawable)>,
}

impl RouteExplorer {
    pub fn new(ctx: &mut EventCtx, app: &App, start: TripEndpoint) -> Box<dyn State<App>> {
        Box::new(RouteExplorer {
            start,
            goal: None,
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Route explorer").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                ctx.style()
                    .btn_outline
                    .text("All routes")
                    .hotkey(Key::A)
                    .build_def(ctx),
                params_to_controls(ctx, TripMode::Bike, &app.primary.map.routing_params())
                    .named("params"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        })
    }

    fn recalc_paths(&mut self, ctx: &mut EventCtx, app: &App) {
        let (mode, params) = controls_to_params(&self.panel);

        if let Some((ref goal, _, ref mut preview)) = self.goal {
            *preview = Drawable::empty(ctx);
            if let Some(polygon) =
                TripEndpoint::path_req(self.start.clone(), goal.clone(), mode, &app.primary.map)
                    .and_then(|req| app.primary.map.pathfind_with_params(req, &params).ok())
                    .and_then(|path| path.trace(&app.primary.map))
                    .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS))
            {
                *preview = GeomBatch::from(vec![(Color::PURPLE, polygon)]).upload(ctx);
            }
        }
    }
}

impl State<App> for RouteExplorer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "bikes" => {
                    let controls =
                        params_to_controls(ctx, TripMode::Bike, app.primary.map.routing_params());
                    self.panel.replace(ctx, "params", controls);
                    self.recalc_paths(ctx, app);
                }
                "cars" => {
                    let controls =
                        params_to_controls(ctx, TripMode::Drive, app.primary.map.routing_params());
                    self.panel.replace(ctx, "params", controls);
                    self.recalc_paths(ctx, app);
                }
                "pedestrians" => {
                    let controls =
                        params_to_controls(ctx, TripMode::Walk, app.primary.map.routing_params());
                    self.panel.replace(ctx, "params", controls);
                    self.recalc_paths(ctx, app);
                }
                "All routes" => {
                    return Transition::Replace(AllRoutesExplorer::new(ctx, app));
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                self.recalc_paths(ctx, app);
            }
            _ => {}
        }

        if self
            .goal
            .as_ref()
            .map(|(_, confirmed, _)| *confirmed)
            .unwrap_or(false)
        {
            return Transition::Keep;
        }

        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_unzoomed_everything(ctx);
            if match app.primary.current_selection {
                Some(ID::Intersection(i)) => !app.primary.map.get_i(i).is_border(),
                Some(ID::Building(_)) => false,
                _ => true,
            } {
                app.primary.current_selection = None;
            }
        }
        if let Some(hovering) = match app.primary.current_selection {
            Some(ID::Intersection(i)) => Some(TripEndpoint::Border(i)),
            Some(ID::Building(b)) => Some(TripEndpoint::Bldg(b)),
            None => None,
            _ => unreachable!(),
        } {
            if self.start != hovering {
                if self
                    .goal
                    .as_ref()
                    .map(|(to, _, _)| to != &hovering)
                    .unwrap_or(true)
                {
                    self.goal = Some((hovering, false, Drawable::empty(ctx)));
                    self.recalc_paths(ctx, app);
                }
            } else {
                self.goal = None;
            }
        } else {
            self.goal = None;
        }

        if let Some((_, ref mut confirmed, _)) = self.goal {
            if app.per_obj.left_click(ctx, "end here") {
                app.primary.current_selection = None;
                *confirmed = true;
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);

        g.draw_polygon(
            Color::BLUE.alpha(0.8),
            match self.start {
                TripEndpoint::Border(i) => app.primary.map.get_i(i).polygon.clone(),
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).polygon.clone(),
                TripEndpoint::SuddenlyAppear(_) => unreachable!(),
            },
        );
        if let Some((ref endpt, _, ref draw)) = self.goal {
            g.draw_polygon(
                Color::GREEN.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i) => app.primary.map.get_i(*i).polygon.clone(),
                    TripEndpoint::Bldg(b) => app.primary.map.get_b(*b).polygon.clone(),
                    TripEndpoint::SuddenlyAppear(_) => unreachable!(),
                },
            );
            g.redraw(draw);
        }
    }
}

fn params_to_controls(ctx: &mut EventCtx, mode: TripMode, params: &RoutingParams) -> Widget {
    let mut rows = vec![Widget::custom_row(vec![
        ctx.style()
            .btn_plain
            .icon("system/assets/meters/bike.svg")
            .disabled(mode == TripMode::Bike)
            .build_widget(ctx, "bikes"),
        ctx.style()
            .btn_plain
            .icon("system/assets/meters/car.svg")
            .disabled(mode == TripMode::Drive)
            .build_widget(ctx, "cars"),
        ctx.style()
            .btn_plain
            .icon("system/assets/meters/pedestrian.svg")
            .disabled(mode == TripMode::Walk)
            .build_widget(ctx, "pedestrians"),
    ])
    .evenly_spaced()];
    if mode == TripMode::Drive || mode == TripMode::Bike {
        rows.push(Widget::row(vec![
            "Unprotected turn penalty:"
                .text_widget(ctx)
                .margin_right(20),
            Spinner::widget(
                ctx,
                (1, 100),
                (params.unprotected_turn_penalty * 10.0) as isize,
            )
            .named("unprotected turn penalty"),
        ]));
    }
    if mode == TripMode::Bike {
        // TODO Spinners that natively understand a floating point range with a given precision
        rows.push(Widget::row(vec![
            "Bike lane penalty:".text_widget(ctx).margin_right(20),
            Spinner::widget(ctx, (0, 20), (params.bike_lane_penalty * 10.0) as isize)
                .named("bike lane penalty"),
        ]));
        rows.push(Widget::row(vec![
            "Bus lane penalty:".text_widget(ctx).margin_right(20),
            Spinner::widget(ctx, (0, 20), (params.bus_lane_penalty * 10.0) as isize)
                .named("bus lane penalty"),
        ]));
        rows.push(Widget::row(vec![
            "Driving lane penalty:".text_widget(ctx).margin_right(20),
            Spinner::widget(ctx, (0, 20), (params.driving_lane_penalty * 10.0) as isize)
                .named("driving lane penalty"),
        ]));
    }
    Widget::col(rows)
}

fn controls_to_params(panel: &Panel) -> (TripMode, RoutingParams) {
    let mut params = RoutingParams::default();
    if !panel.is_button_enabled("cars") {
        params.unprotected_turn_penalty = panel.spinner("unprotected turn penalty") as f64 / 10.0;
        return (TripMode::Drive, params);
    }
    if !panel.is_button_enabled("pedestrians") {
        return (TripMode::Walk, params);
    }
    params.unprotected_turn_penalty = panel.spinner("unprotected turn penalty") as f64 / 10.0;
    params.bike_lane_penalty = panel.spinner("bike lane penalty") as f64 / 10.0;
    params.bus_lane_penalty = panel.spinner("bus lane penalty") as f64 / 10.0;
    params.driving_lane_penalty = panel.spinner("driving lane penalty") as f64 / 10.0;
    (TripMode::Bike, params)
}

/// See how live-tuned routing parameters affect all requests for the current scenario.
struct AllRoutesExplorer {
    panel: Panel,
    requests: Vec<PathRequest>,
    baseline_counts: Counter<RoadID>,

    current_counts: Counter<RoadID>,
    unzoomed: Drawable,
    zoomed: Drawable,
    tooltip: Option<Text>,
}

impl AllRoutesExplorer {
    fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        // Tuning the differential scale is hard enough; always use day mode.
        app.change_color_scheme(ctx, ColorSchemeChoice::DayMode);

        let (requests, baseline_counts) =
            ctx.loading_screen("calculate baseline paths", |_, mut timer| {
                let map = &app.primary.map;
                let requests = timer
                    .parallelize(
                        "predict route requests",
                        Parallelism::Fastest,
                        app.primary.sim.all_trip_info(),
                        |(_, trip)| TripEndpoint::path_req(trip.start, trip.end, trip.mode, map),
                    )
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                let baseline_counts = calculate_demand(app, &requests, &mut timer);
                (requests, baseline_counts)
            });
        let current_counts = baseline_counts.clone();

        // Start by showing the original counts, not relative to anything
        let mut colorer = ColorNetwork::new(app);
        colorer.ranked_roads(current_counts.clone(), &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        Box::new(AllRoutesExplorer {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("All routes explorer").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                format!("{} total requests", prettyprint_usize(requests.len())).text_widget(ctx),
                params_to_controls(ctx, TripMode::Bike, app.primary.map.routing_params())
                    .named("params"),
                ctx.style()
                    .btn_outline
                    .text("Calculate differential demand")
                    .build_def(ctx),
                ctx.style()
                    .btn_solid_destructive
                    .text("keep changed params")
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            requests,
            baseline_counts,
            current_counts,
            unzoomed,
            zoomed,
            tooltip: None,
        })
    }
}

impl State<App> for AllRoutesExplorer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    ctx.loading_screen("revert routing params to defaults", |_, mut timer| {
                        app.primary
                            .map
                            .hack_override_routing_params(RoutingParams::default(), &mut timer);
                    });
                    return Transition::Pop;
                }
                "keep changed params" => {
                    // This is handy for seeing the effects on a real simulation without rebuilding
                    // the map.
                    ctx.loading_screen("update routing params", |_, mut timer| {
                        let (_, params) = controls_to_params(&self.panel);
                        app.primary
                            .map
                            .hack_override_routing_params(params, &mut timer);
                    });
                    return Transition::Pop;
                }
                "bikes" => {
                    let controls =
                        params_to_controls(ctx, TripMode::Bike, app.primary.map.routing_params());
                    self.panel.replace(ctx, "params", controls);
                }
                "cars" => {
                    let controls =
                        params_to_controls(ctx, TripMode::Drive, app.primary.map.routing_params());
                    self.panel.replace(ctx, "params", controls);
                }
                "pedestrians" => {
                    let controls =
                        params_to_controls(ctx, TripMode::Walk, app.primary.map.routing_params());
                    self.panel.replace(ctx, "params", controls);
                }
                "Calculate differential demand" => {
                    ctx.loading_screen(
                        "calculate differential demand due to routing params",
                        |ctx, mut timer| {
                            let (_, params) = controls_to_params(&self.panel);
                            app.primary
                                .map
                                .hack_override_routing_params(params, &mut timer);
                            self.current_counts = calculate_demand(app, &self.requests, &mut timer);

                            // Calculate the difference
                            let mut colorer = ColorNetwork::new(app);
                            // TODO If this works well, promote it alongside DivergingScale
                            let more = &app.cs.good_to_bad_red;
                            let less = &app.cs.good_to_bad_green;
                            let comparisons = self
                                .baseline_counts
                                .clone()
                                .compare(self.current_counts.clone());
                            // Find the biggest gain/loss
                            let diff = comparisons
                                .iter()
                                .map(|(_, after, before)| {
                                    ((*after as isize) - (*before as isize)).abs() as usize
                                })
                                .max()
                                .unwrap() as f64;
                            for (r, before, after) in comparisons {
                                if after < before {
                                    colorer.add_r(r, less.eval((before - after) as f64 / diff));
                                } else if before < after {
                                    colorer.add_r(r, more.eval((after - before) as f64 / diff));
                                }
                            }
                            let (unzoomed, zoomed) = colorer.build(ctx);
                            self.unzoomed = unzoomed;
                            self.zoomed = zoomed;
                        },
                    );
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if ctx.redo_mouseover() {
            self.tooltip = None;
            if let Some(ID::Road(r)) = app.mouseover_unzoomed_roads_and_intersections(ctx) {
                let baseline = self.baseline_counts.get(r);
                let current = self.current_counts.get(r);
                let mut txt = Text::new();
                txt.append_all(cmp_count(current, baseline));
                txt.add(format!("{} baseline", prettyprint_usize(baseline)));
                txt.add(format!("{} now", prettyprint_usize(current)));
                self.tooltip = Some(txt);
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
}

fn calculate_demand(app: &App, requests: &Vec<PathRequest>, timer: &mut Timer) -> Counter<RoadID> {
    let map = &app.primary.map;
    let paths = timer
        .parallelize("pathfind", Parallelism::Fastest, requests.clone(), |req| {
            map.pathfind(req)
        })
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let mut counter = Counter::new();
    timer.start_iter("compute demand", paths.len());
    for path in paths {
        timer.next();
        for step in path.get_steps() {
            if let Traversable::Lane(l) = step.as_traversable() {
                counter.inc(app.primary.map.get_l(l).parent);
            }
        }
    }
    counter
}

fn cmp_count(after: usize, before: usize) -> Vec<TextSpan> {
    if after == before {
        vec![Line("same")]
    } else if after < before {
        vec![
            Line(prettyprint_usize(before - after)).fg(Color::GREEN),
            Line(" less"),
        ]
    } else if after > before {
        vec![
            Line(prettyprint_usize(after - before)).fg(Color::RED),
            Line(" more"),
        ]
    } else {
        unreachable!()
    }
}

use abstutil::Timer;
use geom::{Polygon, Pt2D};
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::{BuildingID, NORMAL_LANE_THICKNESS};
use sim::{IndividTrip, PersonSpec, Scenario, TripEndpoint, TripMode, TripPurpose};
use widgetry::{
    Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, Spinner,
    State, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::CommonState;
use crate::debug::PathCostDebugger;

pub struct AgentSpawner {
    panel: Panel,
    start: Option<(TripEndpoint, Pt2D)>,
    // (goal, point on the map, feasible path, draw the path). Even if we can't draw the path,
    // remember if the path exists at all.
    goal: Option<(TripEndpoint, Pt2D, bool, Option<Polygon>)>,
    confirmed: bool,
}

impl AgentSpawner {
    pub fn new(ctx: &mut EventCtx, app: &App, start: Option<BuildingID>) -> Box<dyn State<App>> {
        let mut spawner = AgentSpawner {
            start: None,
            goal: None,
            confirmed: false,
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("New trip").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                "Click a building or border to specify start"
                    .text_widget(ctx)
                    .named("instructions"),
                Widget::row(vec![
                    "Type of trip:".text_widget(ctx),
                    Widget::dropdown(
                        ctx,
                        "mode",
                        TripMode::Drive,
                        TripMode::all()
                            .into_iter()
                            .map(|m| Choice::new(m.ongoing_verb(), m))
                            .collect(),
                    ),
                ]),
                Widget::row(vec![
                    "Number of trips:".text_widget(ctx),
                    Spinner::widget(ctx, "number", (1, 1000), 1),
                ]),
                if app.opts.dev {
                    ctx.style()
                        .btn_plain_destructive
                        .text("Debug all costs")
                        .build_def(ctx)
                } else {
                    Widget::nothing()
                },
                ctx.style()
                    .btn_solid_primary
                    .text("Confirm")
                    .disabled(true)
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        };
        if let Some(b) = start {
            let endpt = TripEndpoint::Bldg(b);
            let pt = endpt.pt(&app.primary.map);
            spawner.start = Some((endpt, pt));
            spawner.panel.replace(
                ctx,
                "instructions",
                "Click a building or border to specify end".text_widget(ctx),
            );
        }
        Box::new(spawner)
    }
}

impl State<App> for AgentSpawner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Confirm" => {
                    let map = &app.primary.map;
                    let mut scenario = Scenario::empty(map, "one-shot");
                    let from = self.start.take().unwrap().0;
                    let to = self.goal.take().unwrap().0;
                    for _ in 0..self.panel.spinner("number") as usize {
                        scenario.people.push(PersonSpec {
                            orig_id: None,
                            origin: from,
                            trips: vec![IndividTrip::new(
                                app.primary.sim.time(),
                                TripPurpose::Shopping,
                                to,
                                self.panel.dropdown_value("mode"),
                            )],
                        });
                    }
                    let mut rng = app.primary.current_flags.sim_flags.make_rng();
                    scenario.instantiate(
                        &mut app.primary.sim,
                        map,
                        &mut rng,
                        &mut Timer::new("spawn trip"),
                    );
                    app.primary.sim.tiny_step(map, &mut app.primary.sim_cb);
                    app.recalculate_current_selection(ctx);
                    return Transition::Pop;
                }
                "Debug all costs" => {
                    if let Some(state) = self
                        .goal
                        .as_ref()
                        .and_then(|(to, _, _, _)| {
                            TripEndpoint::path_req(
                                self.start.unwrap().0,
                                *to,
                                self.panel.dropdown_value("mode"),
                                &app.primary.map,
                            )
                        })
                        .and_then(|req| app.primary.map.pathfind(req).ok())
                        .and_then(|path| {
                            path.trace(&app.primary.map).map(|pl| {
                                (
                                    path.get_req().clone(),
                                    pl.make_polygons(NORMAL_LANE_THICKNESS),
                                )
                            })
                        })
                        .and_then(|(req, draw_path)| {
                            PathCostDebugger::maybe_new(ctx, app, req, draw_path)
                        })
                    {
                        return Transition::Push(state);
                    } else {
                        return Transition::Push(PopupMsg::new(
                            ctx,
                            "Error",
                            vec!["Couldn't launch cost debugger for some reason"],
                        ));
                    }
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                // We need to recalculate the path to see if this is sane. Otherwise we could trick
                // a pedestrian into wandering on/off a highway border.
                if self.goal.is_some() {
                    let to = self.goal.as_ref().unwrap().0;
                    if let Some(path) = TripEndpoint::path_req(
                        self.start.unwrap().0,
                        to,
                        self.panel.dropdown_value("mode"),
                        &app.primary.map,
                    )
                    .and_then(|req| app.primary.map.pathfind(req).ok())
                    {
                        self.goal = Some((
                            to,
                            to.pt(&app.primary.map),
                            true,
                            path.trace(&app.primary.map)
                                .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS)),
                        ));
                    } else {
                        self.goal = None;
                        self.confirmed = false;
                        self.panel.replace(
                            ctx,
                            "instructions",
                            "Click a building or border to specify end".text_widget(ctx),
                        );
                        self.panel.replace(
                            ctx,
                            "Confirm",
                            ctx.style()
                                .btn_solid_primary
                                .text("Confirm")
                                .disabled(true)
                                .build_def(ctx),
                        );
                    }
                }
            }
            _ => {}
        }

        ctx.canvas_movement();
        let map = &app.primary.map;

        if self.confirmed {
            return Transition::Keep;
        }

        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_unzoomed_everything(ctx);
            if match app.primary.current_selection {
                Some(ID::Intersection(i)) => !map.get_i(i).is_border(),
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
            if self.start.is_none() && app.per_obj.left_click(ctx, "start here") {
                self.start = Some((hovering, hovering.pt(map)));
                self.panel.replace(
                    ctx,
                    "instructions",
                    "Click a building or border to specify end".text_widget(ctx),
                );
            } else if self.start.is_some() && self.start.map(|(x, _)| x != hovering).unwrap_or(true)
            {
                if self
                    .goal
                    .as_ref()
                    .map(|(to, _, _, _)| to != &hovering)
                    .unwrap_or(true)
                {
                    if let Some(path) = TripEndpoint::path_req(
                        self.start.unwrap().0,
                        hovering,
                        self.panel.dropdown_value("mode"),
                        map,
                    )
                    .and_then(|req| map.pathfind(req).ok())
                    {
                        self.goal = Some((
                            hovering,
                            hovering.pt(map),
                            true,
                            path.trace(map)
                                .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS)),
                        ));
                    } else {
                        // Don't constantly recalculate a failed path
                        self.goal = Some((hovering, hovering.pt(map), false, None));
                    }
                }

                if self.goal.as_ref().map(|(_, _, ok, _)| *ok).unwrap_or(false)
                    && app.per_obj.left_click(ctx, "end here")
                {
                    app.primary.current_selection = None;
                    self.confirmed = true;
                    self.panel.replace(
                        ctx,
                        "instructions",
                        "Confirm the trip settings".text_widget(ctx),
                    );
                    self.panel.replace(
                        ctx,
                        "Confirm",
                        ctx.style()
                            .btn_solid_primary
                            .text("Confirm")
                            .hotkey(Key::Enter)
                            .build_def(ctx),
                    );
                }
            }
        } else {
            self.goal = None;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);

        if let Some((_, center)) = self.start {
            map_gui::tools::start_marker(g, center, 2.0).draw(g);
        }
        if let Some((_, center, _, ref path_poly)) = self.goal {
            map_gui::tools::goal_marker(g, center, 2.0).draw(g);
            if let Some(p) = path_poly {
                g.draw_polygon(Color::PURPLE, p.clone());
            }
        }
    }
}

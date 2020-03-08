use crate::app::{App, ShowEverything};
use crate::colors;
use crate::common::ColorLegend;
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::ID;
use crate::managed::WrappedComposite;
use crate::render::{dashed_lines, draw_signal_phase, make_signal_diagram, DrawOptions, DrawTurn};
use ezgui::{
    hotkey, Button, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, ManagedWidget, Outcome, Text, VerticalAlignment,
};
use geom::{Distance, Polygon, Time};
use map_model::{IntersectionID, LaneID, TurnType};
use sim::{AgentID, DontDrawAgents};

// TODO Misnomer. Kind of just handles temporary hovering things now.
pub enum TurnCyclerState {
    Inactive,
    ShowRoute(AgentID, Time, Drawable),
}

impl TurnCyclerState {
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        match app.primary.current_selection {
            Some(ID::Lane(id)) if !app.primary.map.get_turns_from_lane(id).is_empty() => {
                if app
                    .per_obj
                    .action(ctx, Key::Z, "explore turns from this lane")
                {
                    return Some(Transition::Push(Box::new(TurnExplorer {
                        l: id,
                        idx: 0,
                        composite: TurnExplorer::make_panel(ctx, app, id, 0),
                    })));
                }
            }
            Some(ID::Intersection(i)) => {
                if let Some(ref signal) = app.primary.map.maybe_get_traffic_signal(i) {
                    if app
                        .per_obj
                        .action(ctx, Key::F, "explore traffic signal details")
                    {
                        app.primary.current_selection = None;
                        let (idx, _, _) =
                            signal.current_phase_and_remaining_time(app.primary.sim.time());
                        return Some(Transition::Push(Box::new(ShowTrafficSignal {
                            i,
                            composite: make_signal_diagram(ctx, app, i, idx, false),
                            current_phase: idx,
                        })));
                    }
                }
                *self = TurnCyclerState::Inactive;
            }
            Some(ref id) => {
                if let Some(agent) = id.agent_id() {
                    let now = app.primary.sim.time();
                    let recalc = match self {
                        TurnCyclerState::ShowRoute(a, t, _) => agent != *a || now != *t,
                        _ => true,
                    };
                    if recalc {
                        if let Some(trace) =
                            app.primary.sim.trace_route(agent, &app.primary.map, None)
                        {
                            let mut batch = GeomBatch::new();
                            batch.extend(
                                app.cs.get_def("route", Color::ORANGE.alpha(0.5)),
                                dashed_lines(
                                    &trace,
                                    Distance::meters(0.75),
                                    Distance::meters(1.0),
                                    Distance::meters(0.4),
                                ),
                            );
                            *self = TurnCyclerState::ShowRoute(agent, now, batch.upload(ctx));
                        }
                    }
                } else {
                    *self = TurnCyclerState::Inactive;
                }
            }
            _ => {
                *self = TurnCyclerState::Inactive;
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, _: &App) {
        match self {
            TurnCyclerState::Inactive => {}
            TurnCyclerState::ShowRoute(_, _, ref d) => {
                g.redraw(d);
            }
        }
    }
}

struct ShowTrafficSignal {
    i: IntersectionID,
    composite: Composite,
    current_phase: usize,
}

impl State for ShowTrafficSignal {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        // TODO Buttons for these...
        if self.current_phase != 0 && ctx.input.new_was_pressed(&hotkey(Key::UpArrow).unwrap()) {
            self.change_phase(self.current_phase - 1, app, ctx);
        }

        if self.current_phase != app.primary.map.get_traffic_signal(self.i).phases.len() - 1
            && ctx.input.new_was_pressed(&hotkey(Key::DownArrow).unwrap())
        {
            self.change_phase(self.current_phase + 1, app, ctx);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => {
                    self.change_phase(x["phase ".len()..].parse::<usize>().unwrap() - 1, app, ctx);
                }
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.suppress_traffic_signal_details.push(self.i);
        app.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());
        let mut batch = GeomBatch::new();
        draw_signal_phase(
            g.prerender,
            &app.primary.map.get_traffic_signal(self.i).phases[self.current_phase],
            self.i,
            None,
            &mut batch,
            app,
            app.opts.traffic_signal_style.clone(),
        );
        batch.draw(g);

        self.composite.draw(g);
    }
}

impl ShowTrafficSignal {
    fn change_phase(&mut self, idx: usize, app: &App, ctx: &mut EventCtx) {
        if self.current_phase != idx {
            self.current_phase = idx;
            self.composite = make_signal_diagram(ctx, app, self.i, self.current_phase, false);
            self.composite
                .scroll_to_member(ctx, format!("phase {}", idx + 1));
        }
    }
}

struct TurnExplorer {
    l: LaneID,
    // 0 means all turns, otherwise one particular turn
    idx: usize,
    composite: Composite,
}

impl State for TurnExplorer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "previous turn" => {
                    if self.idx != 0 {
                        self.idx -= 1;
                        self.composite = TurnExplorer::make_panel(ctx, app, self.l, self.idx);
                    }
                }
                "next turn" => {
                    if self.idx != app.primary.map.get_turns_from_lane(self.l).len() {
                        self.idx += 1;
                        self.composite = TurnExplorer::make_panel(ctx, app, self.l, self.idx);
                    }
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        {
            let l = app.primary.map.get_l(self.l);
            opts.suppress_traffic_signal_details.push(l.src_i);
            opts.suppress_traffic_signal_details.push(l.dst_i);
        }
        app.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());

        if self.idx == 0 {
            for turn in &app.primary.map.get_turns_from_lane(self.l) {
                DrawTurn::draw_full(turn, g, color_turn_type(turn.turn_type, app).alpha(0.5));
            }
        } else {
            let current = &app.primary.map.get_turns_from_lane(self.l)[self.idx - 1];
            DrawTurn::draw_full(current, g, app.cs.get_def("current turn", Color::GREEN));

            let mut batch = GeomBatch::new();
            for t in app.primary.map.get_turns_in_intersection(current.id.parent) {
                if current.conflicts_with(t) {
                    DrawTurn::draw_dashed(
                        t,
                        &mut batch,
                        app.cs.get_def("conflicting turn", Color::RED.alpha(0.8)),
                    );
                }
            }
            batch.draw(g);
        }

        self.composite.draw(g);
    }
}

impl TurnExplorer {
    fn make_panel(ctx: &mut EventCtx, app: &App, l: LaneID, idx: usize) -> Composite {
        let num_turns = app.primary.map.get_turns_from_lane(l).len();

        let mut col = vec![ManagedWidget::row(vec![
            ManagedWidget::draw_text(
                ctx,
                Text::from(
                    Line(format!(
                        "Turns from {}",
                        app.primary.map.get_parent(l).get_name()
                    ))
                    .size(26),
                ),
            )
            .margin(5),
            ManagedWidget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            )
            .margin(5),
            ManagedWidget::draw_text(
                ctx,
                Text::from(Line(format!("{}/{}", idx, num_turns)).size(20)),
            )
            .margin(5)
            .centered_vert(),
            if idx == 0 {
                Button::inactive_button(ctx, "<")
            } else {
                WrappedComposite::nice_text_button(
                    ctx,
                    Text::from(Line("<")),
                    hotkey(Key::LeftArrow),
                    "previous turn",
                )
            }
            .margin(5),
            if idx == num_turns {
                Button::inactive_button(ctx, ">")
            } else {
                WrappedComposite::nice_text_button(
                    ctx,
                    Text::from(Line(">")),
                    hotkey(Key::RightArrow),
                    "next turn",
                )
            }
            .margin(5),
            WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)),
        ])];
        if idx == 0 {
            if app.primary.map.get_l(l).is_sidewalk() {
                col.push(ColorLegend::row(
                    ctx,
                    app.cs.get("crosswalk turn"),
                    "crosswalk",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    app.cs.get("shared sidewalk corner turn"),
                    "sidewalk connection",
                ));
            } else {
                col.push(ColorLegend::row(
                    ctx,
                    app.cs.get("straight turn"),
                    "straight",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    app.cs.get("right turn"),
                    "right turn",
                ));
                col.push(ColorLegend::row(ctx, app.cs.get("left turn"), "left turn"));
                col.push(ColorLegend::row(
                    ctx,
                    app.cs.get("change lanes left turn"),
                    "straight, but lane-change left",
                ));
                col.push(ColorLegend::row(
                    ctx,
                    app.cs.get("change lanes right turn"),
                    "straight, but lane-change right",
                ));
            }
        } else {
            col.push(ColorLegend::row(
                ctx,
                app.cs.get("current turn"),
                "current turn",
            ));
            col.push(ColorLegend::row(
                ctx,
                app.cs.get("conflicting turn"),
                "conflicting turn",
            ));
        }

        Composite::new(ManagedWidget::col(col).bg(colors::PANEL_BG))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx)
    }
}

fn color_turn_type(t: TurnType, app: &App) -> Color {
    match t {
        TurnType::SharedSidewalkCorner => {
            app.cs.get_def("shared sidewalk corner turn", Color::BLACK)
        }
        TurnType::Crosswalk => app.cs.get_def("crosswalk turn", Color::WHITE),
        TurnType::Straight => app.cs.get_def("straight turn", Color::BLUE),
        TurnType::LaneChangeLeft => app.cs.get_def("change lanes left turn", Color::CYAN),
        TurnType::LaneChangeRight => app.cs.get_def("change lanes right turn", Color::PURPLE),
        TurnType::Right => app.cs.get_def("right turn", Color::GREEN),
        TurnType::Left => app.cs.get_def("left turn", Color::RED),
    }
}

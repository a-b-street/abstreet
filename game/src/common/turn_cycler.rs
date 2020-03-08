use crate::app::{App, ShowEverything};
use crate::game::{DrawBaselayer, State, Transition};
use crate::helpers::ID;
use crate::render::{dashed_lines, draw_signal_phase, make_signal_diagram, DrawOptions, DrawTurn};
use ezgui::{hotkey, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Outcome};
use geom::{Distance, Time};
use map_model::{IntersectionID, LaneID, Map, TurnType};
use sim::{AgentID, DontDrawAgents};

// TODO Misnomer. Kind of just handles temporary hovering things now.
pub enum TurnCyclerState {
    Inactive,
    ShowLane(LaneID),
    ShowRoute(AgentID, Time, Drawable),
    CycleTurns(LaneID, usize),
}

impl TurnCyclerState {
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        match app.primary.current_selection {
            Some(ID::Lane(id)) if !app.primary.map.get_turns_from_lane(id).is_empty() => {
                if let TurnCyclerState::CycleTurns(current, idx) = self {
                    if *current != id {
                        *self = TurnCyclerState::ShowLane(id);
                    } else if app
                        .per_obj
                        .action(ctx, Key::Z, "cycle through this lane's turns")
                    {
                        *self = TurnCyclerState::CycleTurns(id, *idx + 1);
                    }
                } else {
                    *self = TurnCyclerState::ShowLane(id);
                    if app
                        .per_obj
                        .action(ctx, Key::Z, "cycle through this lane's turns")
                    {
                        *self = TurnCyclerState::CycleTurns(id, 0);
                    }
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

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        match self {
            TurnCyclerState::Inactive => {}
            TurnCyclerState::ShowLane(l) => {
                for turn in &app.primary.map.get_turns_from_lane(*l) {
                    DrawTurn::draw_full(turn, g, color_turn_type(turn.turn_type, app).alpha(0.5));
                }
            }
            TurnCyclerState::CycleTurns(l, idx) => {
                let turns = app.primary.map.get_turns_from_lane(*l);
                let current = turns[*idx % turns.len()];
                DrawTurn::draw_full(current, g, color_turn_type(current.turn_type, app));

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
            TurnCyclerState::ShowRoute(_, _, ref d) => {
                g.redraw(d);
            }
        }
    }

    pub fn suppress_traffic_signal_details(&self, map: &Map) -> Option<IntersectionID> {
        match self {
            TurnCyclerState::ShowLane(l) | TurnCyclerState::CycleTurns(l, _) => {
                Some(map.get_l(*l).dst_i)
            }
            TurnCyclerState::ShowRoute(_, _, _) | TurnCyclerState::Inactive => None,
        }
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
        opts.suppress_traffic_signal_details = Some(self.i);
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

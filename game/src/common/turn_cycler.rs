use crate::game::{State, Transition};
use crate::helpers::plain_list_names;
use crate::helpers::ID;
use crate::options::TrafficSignalStyle;
use crate::render::{dashed_lines, draw_signal_phase, DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use ezgui::{
    hotkey, Button, Color, Composite, DrawBoth, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, ModalMenu, Outcome, Text, VerticalAlignment,
};
use geom::{Distance, Polygon, Time};
use map_model::{IntersectionID, LaneID, Map, TurnType};
use sim::{AgentID, DontDrawAgents};
use std::collections::BTreeSet;

// TODO Misnomer. Kind of just handles temporary hovering things now.
pub enum TurnCyclerState {
    Inactive,
    ShowLane(LaneID),
    ShowRoute(AgentID, Time, Drawable),
    CycleTurns(LaneID, usize),
}

impl TurnCyclerState {
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        match ui.primary.current_selection {
            Some(ID::Lane(id)) if !ui.primary.map.get_turns_from_lane(id).is_empty() => {
                if let TurnCyclerState::CycleTurns(current, idx) = self {
                    if *current != id {
                        *self = TurnCyclerState::ShowLane(id);
                    } else if ui
                        .per_obj
                        .action(ctx, Key::Z, "cycle through this lane's turns")
                    {
                        *self = TurnCyclerState::CycleTurns(id, *idx + 1);
                    }
                } else {
                    *self = TurnCyclerState::ShowLane(id);
                    if ui
                        .per_obj
                        .action(ctx, Key::Z, "cycle through this lane's turns")
                    {
                        *self = TurnCyclerState::CycleTurns(id, 0);
                    }
                }
            }
            Some(ID::Intersection(i)) => {
                if let Some(ref signal) = ui.primary.map.maybe_get_traffic_signal(i) {
                    if ui
                        .per_obj
                        .action(ctx, Key::F, "show full traffic signal diagram")
                    {
                        ui.primary.current_selection = None;
                        let (idx, _, _) =
                            signal.current_phase_and_remaining_time(ui.primary.sim.time());
                        return Some(Transition::Push(Box::new(ShowTrafficSignal {
                            menu: ModalMenu::new(
                                "Traffic Signal Diagram",
                                vec![
                                    (hotkey(Key::UpArrow), "select previous phase"),
                                    (hotkey(Key::DownArrow), "select next phase"),
                                    (hotkey(Key::Escape), "quit"),
                                ],
                                ctx,
                            ),
                            diagram: TrafficSignalDiagram::new(i, idx, ui, ctx),
                        })));
                    }
                }
                *self = TurnCyclerState::Inactive;
            }
            Some(ref id) => {
                if let Some(agent) = id.agent_id() {
                    let now = ui.primary.sim.time();
                    let recalc = match self {
                        TurnCyclerState::ShowRoute(a, t, _) => agent != *a || now != *t,
                        _ => true,
                    };
                    if recalc {
                        if let Some(trace) =
                            ui.primary.sim.trace_route(agent, &ui.primary.map, None)
                        {
                            let mut batch = GeomBatch::new();
                            batch.extend(
                                ui.cs.get("route"),
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
                }
            }
            _ => {
                *self = TurnCyclerState::Inactive;
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        match self {
            TurnCyclerState::Inactive => {}
            TurnCyclerState::ShowLane(l) => {
                for turn in &ui.primary.map.get_turns_from_lane(*l) {
                    DrawTurn::draw_full(turn, g, color_turn_type(turn.turn_type, ui).alpha(0.5));
                }
            }
            TurnCyclerState::CycleTurns(l, idx) => {
                let turns = ui.primary.map.get_turns_from_lane(*l);
                let current = turns[*idx % turns.len()];
                DrawTurn::draw_full(current, g, color_turn_type(current.turn_type, ui));

                let mut batch = GeomBatch::new();
                for t in ui.primary.map.get_turns_in_intersection(current.id.parent) {
                    if current.conflicts_with(t) {
                        DrawTurn::draw_dashed(
                            t,
                            &mut batch,
                            ui.cs.get_def("conflicting turn", Color::RED.alpha(0.8)),
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

fn color_turn_type(t: TurnType, ui: &UI) -> Color {
    match t {
        TurnType::SharedSidewalkCorner => {
            ui.cs.get_def("shared sidewalk corner turn", Color::BLACK)
        }
        TurnType::Crosswalk => ui.cs.get_def("crosswalk turn", Color::WHITE),
        TurnType::Straight => ui.cs.get_def("straight turn", Color::BLUE),
        TurnType::LaneChangeLeft => ui.cs.get_def("change lanes left turn", Color::CYAN),
        TurnType::LaneChangeRight => ui.cs.get_def("change lanes right turn", Color::PURPLE),
        TurnType::Right => ui.cs.get_def("right turn", Color::GREEN),
        TurnType::Left => ui.cs.get_def("left turn", Color::RED),
    }
}

struct ShowTrafficSignal {
    menu: ModalMenu,
    // TODO Probably collapse diagram here, like editor did
    diagram: TrafficSignalDiagram,
}

impl State for ShowTrafficSignal {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);
        ctx.canvas_movement();
        if self.menu.action("quit") {
            return Transition::Pop;
        }
        self.diagram.event(ctx, ui, &mut self.menu);
        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = DrawOptions::new();
        opts.suppress_traffic_signal_details = Some(self.diagram.i);
        ui.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());
        let ctx = ui.draw_ctx();
        let mut batch = GeomBatch::new();
        draw_signal_phase(
            &ui.primary.map.get_traffic_signal(self.diagram.i).phases[self.diagram.current_phase],
            self.diagram.i,
            None,
            &mut batch,
            &ctx,
            ctx.opts.traffic_signal_style.clone(),
        );
        batch.draw(g);

        self.diagram.draw(g);
        self.menu.draw(g);
    }
}

struct TrafficSignalDiagram {
    pub i: IntersectionID,
    composite: Composite,
    pub current_phase: usize,
}

impl TrafficSignalDiagram {
    fn new(
        i: IntersectionID,
        current_phase: usize,
        ui: &UI,
        ctx: &mut EventCtx,
    ) -> TrafficSignalDiagram {
        TrafficSignalDiagram {
            i,
            composite: make_diagram(i, current_phase, ui, ctx),
            current_phase,
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI, menu: &mut ModalMenu) {
        if self.current_phase != 0 && menu.action("select previous phase") {
            self.change_phase(self.current_phase - 1, ui, ctx);
        }

        if self.current_phase != ui.primary.map.get_traffic_signal(self.i).phases.len() - 1
            && menu.action("select next phase")
        {
            self.change_phase(self.current_phase + 1, ui, ctx);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                self.change_phase(x["phase ".len()..].parse::<usize>().unwrap() - 1, ui, ctx);
            }
            None => {}
        }
    }

    fn change_phase(&mut self, idx: usize, ui: &UI, ctx: &mut EventCtx) {
        if self.current_phase != idx {
            let preserve_scroll = self.composite.preserve_scroll();
            self.current_phase = idx;
            self.composite = make_diagram(self.i, self.current_phase, ui, ctx);
            self.composite.restore_scroll(ctx, preserve_scroll);
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}

fn make_diagram(i: IntersectionID, selected: usize, ui: &UI, ctx: &mut EventCtx) -> Composite {
    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = ui.primary.map.get_i(i).polygon.get_bounds();
    // Pick a zoom so that we fit some percentage of the screen
    let zoom = 0.2 * ctx.canvas.window_width / bounds.width();
    let bbox = Polygon::rectangle(zoom * bounds.width(), zoom * bounds.height());

    let signal = ui.primary.map.get_traffic_signal(i);
    let mut col = vec![ManagedWidget::draw_text(ctx, {
        let mut txt = Text::new();
        let road_names = ui
            .primary
            .map
            .get_i(i)
            .roads
            .iter()
            .map(|r| ui.primary.map.get_r(*r).get_name())
            .collect::<BTreeSet<_>>();
        // TODO Style inside here. Also 0.4 is manually tuned and pretty wacky, because it
        // assumes default font.
        txt.add_wrapped(plain_list_names(road_names), 0.4 * ctx.canvas.window_width);
        txt.add(Line(format!("{} phases", signal.phases.len())));
        txt.add(Line(format!("Signal offset: {}", signal.offset)));
        txt.add(Line(format!("One cycle lasts {}", signal.cycle_length())));
        txt
    })];
    for (idx, phase) in signal.phases.iter().enumerate() {
        col.push(
            ManagedWidget::row(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line(format!("#{}", idx + 1)))),
                ManagedWidget::draw_text(ctx, Text::from(Line(phase.duration.to_string()))),
            ])
            .margin(5)
            .evenly_spaced(),
        );

        let mut orig_batch = GeomBatch::new();
        draw_signal_phase(
            phase,
            i,
            None,
            &mut orig_batch,
            &ui.draw_ctx(),
            TrafficSignalStyle::Sidewalks,
        );

        let mut normal = GeomBatch::new();
        // TODO Ideally no background here, but we have to force the dimensions of normal and
        // hovered to be the same. For some reason the bbox is slightly different.
        if idx == selected {
            normal.push(Color::RED.alpha(0.15), bbox.clone());
        } else {
            normal.push(Color::CYAN.alpha(0.05), bbox.clone());
        }
        // Move to the origin and apply zoom
        for (color, poly) in orig_batch.consume() {
            normal.push(
                color,
                poly.translate(-bounds.min_x, -bounds.min_y).scale(zoom),
            );
        }

        let mut hovered = GeomBatch::new();
        hovered.push(Color::RED.alpha(0.95), bbox.clone());
        hovered.append(normal.clone());

        col.push(
            ManagedWidget::btn(Button::new(
                DrawBoth::new(ctx, normal, Vec::new()),
                DrawBoth::new(ctx, hovered, Vec::new()),
                None,
                &format!("phase {}", idx + 1),
                bbox.clone(),
            ))
            .margin(5),
        );
    }

    Composite::new(ManagedWidget::col(col).bg(Color::hex("#545454")))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .max_size_percent(30, 90)
        .build(ctx)
}

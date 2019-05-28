use crate::common::Warper;
use crate::helpers::ID;
use crate::render::DrawTurn;
use crate::ui::UI;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text};
use geom::{Distance, Polygon};
use map_model::{Traversable, LANE_THICKNESS};
use sim::AgentID;

pub struct RouteExplorer {
    menu: ModalMenu,
    agent: AgentID,
    steps: Vec<Traversable>,
    entire_trace: Option<Polygon>,
    current: usize,
    warper: Option<Warper>,
}

impl RouteExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<RouteExplorer> {
        let (agent, path) = if true {
            let agent = ui.primary.current_selection.and_then(|id| id.agent_id())?;
            (agent, ui.primary.sim.get_path(agent)?.clone())
        } else {
            use map_model::{LaneID, PathRequest, Position};

            // TODO Temporary for debugging
            let agent = AgentID::Pedestrian(sim::PedestrianID(42));
            let path = ui.primary.map.pathfind(PathRequest {
                start: Position::new(LaneID(4409), Distance::meters(146.9885)),
                end: Position::new(LaneID(8188), Distance::meters(82.4241)),
                can_use_bike_lanes: false,
                can_use_bus_lanes: false,
            });
            (agent, path?)
        };

        if !ctx.input.contextual_action(Key::E, "explore route") {
            return None;
        }

        // TODO Actual start dist
        let entire_trace = path
            .trace(&ui.primary.map, Distance::ZERO, None)
            .map(|pl| pl.make_polygons(LANE_THICKNESS));

        let steps: Vec<Traversable> = path
            .get_steps()
            .iter()
            .map(|step| step.as_traversable())
            .collect();
        Some(RouteExplorer {
            menu: ModalMenu::new(
                &format!("Route Explorer for {:?}", agent),
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::Dot), "next step"),
                    (Some(Key::Comma), "prev step"),
                    (Some(Key::F), "first step"),
                    (Some(Key::L), "last step"),
                ],
                ctx,
            ),
            agent,
            current: 0,
            warper: Some(Warper::new(
                ctx,
                steps[0]
                    .dist_along(steps[0].length(&ui.primary.map) / 2.0, &ui.primary.map)
                    .0,
                match steps[0] {
                    Traversable::Lane(l) => ID::Lane(l),
                    Traversable::Turn(t) => ID::Turn(t),
                },
            )),
            steps,
            entire_trace,
        })
    }

    // Done when None
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        if let Some(ref warper) = self.warper {
            if let Some(mode) = warper.event(ctx, ui) {
                return Some(mode);
            }
            self.warper = None;
        }

        let mut txt = Text::prompt(&format!("Route Explorer for {:?}", self.agent));
        txt.add_line(format!("Step {}/{}", self.current + 1, self.steps.len()));
        txt.add_line(format!("{:?}", self.steps[self.current]));
        self.menu.handle_event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return None;
        } else if self.current != self.steps.len() - 1 && self.menu.action("next step") {
            self.current += 1;
        } else if self.current != self.steps.len() - 1 && self.menu.action("last step") {
            self.current = self.steps.len() - 1;
        } else if self.current != 0 && self.menu.action("prev step") {
            self.current -= 1;
        } else if self.current != 0 && self.menu.action("first step") {
            self.current = 0;
        } else {
            return Some(EventLoopMode::InputOnly);
        }
        self.warper = Some(Warper::new(
            ctx,
            self.steps[self.current]
                .dist_along(
                    self.steps[self.current].length(&ui.primary.map) / 2.0,
                    &ui.primary.map,
                )
                .0,
            match self.steps[self.current] {
                Traversable::Lane(l) => ID::Lane(l),
                Traversable::Turn(t) => ID::Turn(t),
            },
        ));

        Some(EventLoopMode::InputOnly)
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(ref poly) = self.entire_trace {
            g.draw_polygon(ui.cs.get_def("entire route", Color::BLUE.alpha(0.2)), poly);
        }

        let color = ui.cs.get_def("current step", Color::RED);
        match self.steps[self.current] {
            Traversable::Lane(l) => {
                g.draw_polygon(color, &ui.primary.draw_map.get_l(l).polygon);
            }
            Traversable::Turn(t) => {
                DrawTurn::draw_full(ui.primary.map.get_t(t), g, color);
            }
        }
        self.menu.draw(g);
    }
}

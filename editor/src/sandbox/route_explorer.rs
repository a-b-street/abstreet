use crate::common::Warper;
use crate::helpers::ID;
use crate::render::DrawTurn;
use crate::ui::UI;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Slider, Text};
use geom::{Distance, Polygon};
use map_model::{Traversable, LANE_THICKNESS};
use sim::AgentID;

pub struct RouteExplorer {
    menu: ModalMenu,
    agent: AgentID,
    steps: Vec<Traversable>,
    entire_trace: Option<Polygon>,
    warper: Option<Warper>,
    slider: Slider,
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
            slider: Slider::new(0, steps.len() - 1),
            steps,
            entire_trace,
        })
    }

    // Done when None
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        // Don't block while we're warping
        let ev_mode = if let Some(ref warper) = self.warper {
            if let Some(mode) = warper.event(ctx, ui) {
                mode
            } else {
                self.warper = None;
                EventLoopMode::InputOnly
            }
        } else {
            EventLoopMode::InputOnly
        };

        let current = self.slider.get_value();

        let mut txt = Text::prompt(&format!("Route Explorer for {:?}", self.agent));
        txt.add_line(format!("Step {}/{}", current + 1, self.steps.len()));
        txt.add_line(format!("{:?}", self.steps[current]));
        self.menu.handle_event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return None;
        } else if current != self.steps.len() - 1 && self.menu.action("next step") {
            self.slider.set_value(ctx, current + 1);
        } else if current != self.steps.len() - 1 && self.menu.action("last step") {
            self.slider.set_value(ctx, self.steps.len() - 1);
        } else if current != 0 && self.menu.action("prev step") {
            self.slider.set_value(ctx, current - 1);
        } else if current != 0 && self.menu.action("first step") {
            self.slider.set_value(ctx, 0);
        } else if self.slider.event(ctx) {
            // Cool, the value changed, so fall-through
        } else {
            return Some(ev_mode);
        }

        let step = self.steps[self.slider.get_value()];
        self.warper = Some(Warper::new(
            ctx,
            step.dist_along(step.length(&ui.primary.map) / 2.0, &ui.primary.map)
                .0,
            match step {
                Traversable::Lane(l) => ID::Lane(l),
                Traversable::Turn(t) => ID::Turn(t),
            },
        ));
        // We just created a new warper, so...
        Some(EventLoopMode::Animation)
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(ref poly) = self.entire_trace {
            g.draw_polygon(ui.cs.get_def("entire route", Color::BLUE.alpha(0.2)), poly);
        }

        let color = ui.cs.get_def("current step", Color::RED);
        match self.steps[self.slider.get_value()] {
            Traversable::Lane(l) => {
                g.draw_polygon(color, &ui.primary.draw_map.get_l(l).polygon);
            }
            Traversable::Turn(t) => {
                DrawTurn::draw_full(ui.primary.map.get_t(t), g, color);
            }
        }
        self.menu.draw(g);
        self.slider.draw(g);
    }
}

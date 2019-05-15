use crate::common::Warper;
use crate::helpers::ID;
use crate::render::DrawTurn;
use crate::ui::UI;
use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Text};
use map_model::Traversable;
use sim::AgentID;

pub struct RouteExplorer {
    menu: ModalMenu,
    agent: AgentID,
    steps: Vec<Traversable>,
    current: usize,
    warper: Option<Warper>,
}

impl RouteExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<RouteExplorer> {
        let agent = ui.primary.current_selection.and_then(|id| id.agent_id())?;
        let path = ui.primary.sim.get_path(agent)?;
        if !ctx.input.contextual_action(Key::E, "explore route") {
            return None;
        }
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
            steps: path
                .get_steps()
                .iter()
                .map(|step| step.as_traversable())
                .collect(),
            current: 0,
            warper: None,
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

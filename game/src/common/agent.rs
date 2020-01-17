use crate::common::route_viewer::RouteViewer;
use crate::common::TripExplorer;
use crate::game::Transition;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Key, ModalMenu};

pub struct AgentTools {
    route_viewer: RouteViewer,
}

impl AgentTools {
    pub fn new() -> AgentTools {
        AgentTools {
            route_viewer: RouteViewer::Inactive,
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &UI,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        self.route_viewer.event(ctx, ui, menu);

        if let Some(trip) = ui
            .primary
            .current_selection
            .as_ref()
            .and_then(|id| id.agent_id())
            .and_then(|agent| ui.primary.sim.agent_to_trip(agent))
        {
            if ui.per_obj.action(ctx, Key::T, format!("explore {}", trip)) {
                return Some(Transition::Push(Box::new(TripExplorer::new(trip, ctx, ui))));
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.route_viewer.draw(g);
    }
}

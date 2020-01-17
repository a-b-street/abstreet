use crate::common::route_viewer::RouteViewer;
use crate::game::Transition;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, ModalMenu};

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
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.route_viewer.draw(g);
    }
}

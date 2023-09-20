use map_model::BuildingID;
use widgetry::{EventCtx, State};

use crate::app::{App, Transition};

pub struct BuildingEditor {

}

impl BuildingEditor {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, b: BuildingID) -> Box<dyn State<App>> {
        BuildingEditor::create(ctx, app, b)
    }

    fn create(
        ctx: &mut EventCtx,
        app: &mut App,
        b: BuildingID,
    ) -> Box<dyn State<App>> {
		todo!()
	}
}

impl State<App> for BuildingEditor {
    fn event(&mut self, ctx: &mut EventCtx, shared_app_state: &mut App) -> Transition {
        todo!()
    }

    fn draw(&self, g: &mut widgetry::GfxCtx, shared_app_state: &App) {
        todo!()
    }
}
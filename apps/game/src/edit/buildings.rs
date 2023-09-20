use map_model::{BuildingID, EditBuilding, EditCmd};
use widgetry::{EventCtx, State};

use crate::app::{App, Transition};

pub struct BuildingEditor {
    b: BuildingID,

    // Undo/redo management
    num_edit_cmds_originally: usize,
    redo_stack: Vec<EditCmd>,
    orig_building_state: EditBuilding,
}

impl BuildingEditor {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, b: BuildingID) -> Box<dyn State<App>> {
        BuildingEditor::create(ctx, app, b)
    }

    fn create(ctx: &mut EventCtx, app: &mut App, b: BuildingID) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let mut editor = BuildingEditor {
            b,

            num_edit_cmds_originally: app.primary.map.get_edits().commands.len(),
            redo_stack: Vec::new(),
            orig_building_state: app.primary.map.get_b_edit(b), // TODO
        };
        // TODO recalc panels?
        Box::new(editor)
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

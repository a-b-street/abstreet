use map_model::{Building, BuildingID, EditBuilding, EditCmd};
use widgetry::{Drawable, EventCtx, GeomBatch, Panel, State, HorizontalAlignment, VerticalAlignment};

use crate::app::{App, Transition};

pub struct BuildingEditor {
    b: BuildingID,

    top_panel: Panel,
    main_panel: Panel,
    fade_irrelevant: Drawable,

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
            top_panel: Panel::empty(ctx),
            main_panel: Panel::empty(ctx),
            fade_irrelevant: Drawable::empty(ctx),

            num_edit_cmds_originally: app.primary.map.get_edits().commands.len(),
            redo_stack: Vec::new(),
            orig_building_state: app.primary.map.get_b_edit(b), // TODO
        };
        editor.recalc_all_panels(ctx, app);
        Box::new(editor)
    }

    fn recalc_all_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        self.main_panel = make_main_panel(ctx, app, self.b);

        self.fade_irrelevant = fade_irrelevant(app, self.b).upload(ctx);
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

fn make_main_panel(ctx: &mut EventCtx, app: &App, b: BuildingID) -> Panel {
    let map = &app.primary.map;
    let current_state = map.get_b_edit(b);
    
    Panel::new_builder().aligned(HorizontalAlignment::Center, VerticalAlignment::TopInset).build(ctx)
}

fn fade_irrelevant(app: &App, b: BuildingID) -> GeomBatch {
    todo!()
}

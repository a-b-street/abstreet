use std::vec;

use map_model::{BuildingID, EditBuilding, EditCmd};
use widgetry::{
    lctrl, Drawable, EventCtx, GeomBatch, HorizontalAlignment, Key, Line, Panel, State,
    VerticalAlignment, Widget,
};

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

        self.top_panel = make_top_panel(
            ctx,
            app,
            self.num_edit_cmds_originally,
            self.redo_stack.is_empty(),
            self.b,
        );

        self.fade_irrelevant = fade_irrelevant(app, self.b).upload(ctx);
    }
}

impl State<App> for BuildingEditor {
    fn event(&mut self, ctx: &mut EventCtx, shared_app_state: &mut App) -> Transition {
        // TODO: jump to building action
        todo!()
    }

    fn draw(&self, g: &mut widgetry::GfxCtx, shared_app_state: &App) {
        todo!()
    }
}

fn make_top_panel(
    ctx: &mut EventCtx,
    app: &App,
    num_edit_cmds_originally: usize,
    no_redo_cmds: bool,
    b: BuildingID,
) -> Panel {
    let map = &app.primary.map;
    let current_state = map.get_b_edit(b);

    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            Line(format!("Edit {}", b)).small_heading().into_widget(ctx),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/location.svg")
                .build_widget(ctx, "jump to building"),
        ]),
        Widget::row(vec![
            ctx.style()
                .btn_solid_primary
                .text("Finish")
                .hotkey(Key::Enter)
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/undo.svg")
                .disabled(map.get_edits().commands.len() == num_edit_cmds_originally)
                .hotkey(lctrl(Key::Z))
                .build_widget(ctx, "undo"),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/redo.svg")
                .disabled(no_redo_cmds)
                // TODO ctrl+shift+Z!
                .hotkey(lctrl(Key::Y))
                .build_widget(ctx, "redo"),
            ctx.style()
                .btn_plain_destructive
                .text("Revert")
                .disabled(
                    current_state
                        == EditBuilding::get_orig_from_osm(map.get_b(b), map.get_config()),
                )
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .text("Cancel")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ]),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_main_panel(ctx: &mut EventCtx, app: &App, b: BuildingID) -> Panel {
    let map = &app.primary.map;
    let current_state = map.get_b_edit(b);

    Panel::new_builder(Widget::col(vec![Widget::row(vec![
        Line("Parking").secondary().into_widget(ctx).centered_vert(),
        Widget::dropdown(ctx, "type", current_state.parking, todo!()).centered_vert(),
        // TODO: edit capacity
    ])]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Center)
    .build(ctx)
}

fn fade_irrelevant(app: &App, b: BuildingID) -> GeomBatch {
    todo!()
}

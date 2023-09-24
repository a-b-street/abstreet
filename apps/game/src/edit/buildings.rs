use std::vec;

use map_model::{BuildingID, EditBuilding, EditCmd, OffstreetParking};
use widgetry::{
    lctrl, EventCtx, HorizontalAlignment, Key, Line, Outcome, Panel, Spinner, State,
    VerticalAlignment, Widget,
};

use crate::{
    app::{App, Transition},
    common::Warping,
    id::ID,
};

use super::can_edit_building_parking;

pub struct BuildingEditor {
    b: BuildingID,

    top_panel: Panel,
    main_panel: Panel,
    // TODO: fade_irrelevant to make things look nicer

    // Undo/redo management
    num_edit_cmds_originally: usize,
    redo_stack: Vec<EditCmd>,
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

            num_edit_cmds_originally: app.primary.map.get_edits().commands.len(),
            redo_stack: Vec::new(),
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
    }
}

impl State<App> for BuildingEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "jump to building" => {
                    return Transition::Push(Warping::new_state(
                        ctx,
                        app.primary.canonical_point(ID::Building(self.b)).unwrap(),
                        Some(10.0),
                        Some(ID::Building(self.b)),
                        &mut app.primary,
                    ))
                }
                _ => unreachable!(),
            }
        }

        match self.main_panel.event(ctx) {
            Outcome::Changed(x) => match x.as_ref() {
                "parking type" => {
                    // TODO allow changing between public and private
                    unimplemented!()
                }
                "parking capacity" => {
                    unimplemented!()
                }
                _ => unreachable!(),
            },
            _ => debug!("main_panel had unhandled outcome"),
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut widgetry::GfxCtx, _: &App) {
        self.top_panel.draw(g);
        self.main_panel.draw(g);
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
    if !can_edit_building_parking(app, b) {
        return Panel::empty(ctx);
    }
    let map = &app.primary.map;
    let current_state = map.get_b_edit(b);
    let current_parking_capacity = match current_state.parking {
        OffstreetParking::PublicGarage(_, _) | OffstreetParking::Private(_, false) => {
            // TODO support editing parking for these cases
            unreachable!()
        }
        OffstreetParking::Private(count, true) => count,
    };
    Panel::new_builder(Widget::col(vec![Widget::row(vec![
        Line("Parking capacity")
            .secondary()
            .into_widget(ctx)
            .centered_vert(),
        Spinner::widget(
            ctx,
            "parking capacity",
            (0, 999_999),
            current_parking_capacity,
            1,
        ),
    ])]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Center)
    .build(ctx)
}

use std::vec;

use map_model::{BuildingID, EditCmd, MapEdits, OffstreetParking};
use widgetry::{
    lctrl, Choice, EventCtx, HorizontalAlignment, Key, Line, Outcome, Panel, Spinner, State,
    TextBox, VerticalAlignment, Widget,
};

use crate::{
    app::{App, Transition},
    common::Warping,
    id::ID,
};

use super::apply_map_edits;

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

    fn compress_edits(&self, app: &App) -> Option<MapEdits> {
        // Compress all of the edits, unless there were 0 or 1 changes
        if app.primary.map.get_edits().commands.len() > self.num_edit_cmds_originally + 2 {
            let mut edits = app.primary.map.get_edits().clone();
            let last_edit = match edits.commands.pop().unwrap() {
                EditCmd::ChangeBuilding { new, .. } => new,
                _ => unreachable!(),
            };
            edits.commands.truncate(self.num_edit_cmds_originally + 1);
            match edits.commands.last_mut().unwrap() {
                EditCmd::ChangeBuilding { ref mut new, .. } => {
                    *new = last_edit;
                }
                _ => unreachable!(),
            }
            return Some(edits);
        }
        None
    }
}

impl State<App> for BuildingEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        let mut panels_need_recalc = false;

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Finish" => {
                    if let Some(edits) = self.compress_edits(app) {
                        apply_map_edits(ctx, app, edits);
                    }
                    return Transition::Pop;
                }
                "Cancel" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    if edits.commands.len() != self.num_edit_cmds_originally {
                        edits.commands.truncate(self.num_edit_cmds_originally);
                        apply_map_edits(ctx, app, edits);
                    }
                    return Transition::Pop;
                }
                "undo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    self.redo_stack.push(edits.commands.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    panels_need_recalc = true;
                }
                "redo" => {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(self.redo_stack.pop().unwrap());
                    apply_map_edits(ctx, app, edits);

                    panels_need_recalc = true;
                }
                "jump to building" => {
                    return Transition::Push(Warping::new_state(
                        ctx,
                        app.primary.canonical_point(ID::Building(self.b)).unwrap(),
                        Some(10.0),
                        Some(ID::Building(self.b)),
                        &mut app.primary,
                    ))
                }
                _ => unreachable!("received unknown clicked key: {}", x),
            }
        }

        match self.main_panel.event(ctx) {
            Outcome::Changed(x) => match x.as_ref() {
                "parking type" => {
                    let parking_type = self.main_panel.dropdown_value("parking type");
                    let parking_capacity: usize = self.main_panel.spinner("parking_capacity");

                    let mut edits = app.primary.map.get_edits().clone();
                    let old = app.primary.map.get_b_edit(self.b);
                    let mut new = old.clone();

                    new.parking = match parking_type {
                        "public" => {
                            // TODO support input public garage name
                            OffstreetParking::PublicGarage("TODO".to_string(), parking_capacity)
                        }
                        "private" => {
                            OffstreetParking::Private(parking_capacity, parking_capacity > 0)
                        }
                        _ => unreachable!("unknown parking type received: {}", parking_type),
                    };

                    edits.commands.push(EditCmd::ChangeBuilding {
                        b: self.b,
                        old,
                        new,
                    });
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    panels_need_recalc = true;
                }
                "parking_capacity" => {
                    let parking_capacity: usize = self.main_panel.spinner("parking_capacity");

                    let mut edits = app.primary.map.get_edits().clone();
                    let old = app.primary.map.get_b_edit(self.b);
                    let mut new = old.clone();
                    new.parking = match old.parking {
                        OffstreetParking::Private(_, has_parking) => {
                            OffstreetParking::Private(parking_capacity, has_parking)
                        }
                        OffstreetParking::PublicGarage(ref name, _) => {
                            OffstreetParking::PublicGarage(name.to_string(), parking_capacity)
                        }
                    };
                    edits.commands.push(EditCmd::ChangeBuilding {
                        b: self.b,
                        old,
                        new,
                    });
                    apply_map_edits(ctx, app, edits);
                    self.redo_stack.clear();

                    panels_need_recalc = true;
                }
                "new_garage_name" => {
                    let new_garage_name = self.main_panel.text_box("new_garage_name");

                    let mut edits = app.primary.map.get_edits().clone();
                    let old = app.primary.map.get_b_edit(self.b);
                    let mut new = old.clone();

                    new.parking = match old.parking {
                        OffstreetParking::Private(_, _) => {
                            unreachable!("Garage name can only be edited if it is public");
                        }
                        OffstreetParking::PublicGarage(_, size) => {
                            OffstreetParking::PublicGarage(new_garage_name, size)
                        }
                    };
                    edits.commands.push(EditCmd::ChangeBuilding {
                        b: self.b,
                        old,
                        new,
                    })
                }
                _ => unreachable!("received unknown change key: {}", x),
            },
            _ => debug!("main_panel had unhandled outcome"),
        }

        if panels_need_recalc {
            self.recalc_all_panels(ctx, app);
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
    let current_parking_capacity = match current_state.parking {
        OffstreetParking::Private(count, true) | OffstreetParking::PublicGarage(_, count) => count,
        OffstreetParking::Private(_, false) => 0,
    };

    let mut fields = vec![
        Widget::row(vec![
            Line("Parking type")
                .secondary()
                .into_widget(ctx)
                .centered_vert(),
            Widget::dropdown(
                ctx,
                "parking type",
                current_state.parking.get_variant_name(),
                parking_type_choices(),
            ),
        ]),
        Widget::row(vec![
            Line("Parking capacity")
                .secondary()
                .into_widget(ctx)
                .centered_vert(),
            Spinner::widget(
                ctx,
                "parking_capacity",
                (0, 999_999),
                current_parking_capacity,
                1,
            ),
        ]),
    ];

    if let OffstreetParking::PublicGarage(name, _) = current_state.parking {
        fields.push(Widget::row(vec![
            Line("Garage Name")
                .secondary()
                .into_widget(ctx)
                .centered_vert(),
            TextBox::widget(ctx, "new_garage_name", name, false, 100),
        ]));
    }

    Panel::new_builder(Widget::col(fields))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Center)
        .build(ctx)
}

fn parking_type_choices() -> Vec<Choice<String>> {
    let choices = vec!["public", "private"];
    choices
        .into_iter()
        .map(|choice| Choice::new(choice.to_string(), choice.to_string()))
        .collect()
}

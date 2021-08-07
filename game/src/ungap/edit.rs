use geom::Distance;
use map_gui::ID;
use map_model::{EditCmd, LaneType};
use widgetry::{
    lctrl, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::Warping;
use crate::edit::{LoadEdits, RoadEditor, SaveEdits};
use crate::sandbox::gameplay::GameplayMode;
use crate::ungap::magnifying::MagnifyingGlass;

const EDITED_COLOR: Color = Color::CYAN;

pub struct QuickEdit {
    top_panel: Panel,
    network_layer: Drawable,
    edits_layer: Drawable,
    magnifying_glass: MagnifyingGlass,

    // edits name, number of commands
    // TODO Brittle -- could undo and add a new command. Add a proper edit counter to map. Refactor
    // with EditMode. Use Cached.
    changelist_key: (String, usize),
}

impl QuickEdit {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let edits = app.primary.map.get_edits();
        Box::new(QuickEdit {
            top_panel: make_top_panel(ctx, app),
            magnifying_glass: MagnifyingGlass::new(ctx, false),
            network_layer: crate::ungap::render_network_layer(ctx, app),
            edits_layer: render_edits(ctx, app),

            changelist_key: (edits.edits_name.clone(), edits.commands.len()),
        })
    }
}

impl State<App> for QuickEdit {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        {
            let edits = app.primary.map.get_edits();
            let changelist_key = (edits.edits_name.clone(), edits.commands.len());
            if self.changelist_key != changelist_key {
                self.changelist_key = changelist_key;
                self.network_layer = crate::ungap::render_network_layer(ctx, app);
                self.edits_layer = render_edits(ctx, app);
                self.top_panel = make_top_panel(ctx, app);
            }
        }

        ctx.canvas_movement();
        self.magnifying_glass.event(ctx, app);

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Open a proposal" => {
                    // Dummy mode, just to allow all edits
                    // TODO Actually, should we make one to express that only road edits are
                    // relevant?
                    let mode = GameplayMode::Freeform(app.primary.map.get_name().clone());

                    // TODO Do we want to do SaveEdits first if unsaved_edits()? We have
                    // auto-saving... and after loading an old "untitled proposal", it looks
                    // unsaved.
                    return Transition::Push(LoadEdits::new_state(ctx, app, mode));
                }
                "Save this proposal" => {
                    return Transition::Push(SaveEdits::new_state(
                        ctx,
                        app,
                        format!("Save \"{}\" as", app.primary.map.get_edits().edits_name),
                        false,
                        Some(Transition::Pop),
                        Box::new(|_, _| {}),
                    ));
                }
                "Sketch a route" => {
                    app.primary.current_selection = None;
                    return Transition::Push(crate::ungap::quick_sketch::QuickSketch::new_state(
                        ctx, app,
                    ));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        // Click to edit a road in detail
        if ctx.redo_mouseover() {
            app.primary.current_selection =
                match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    Some(ID::Road(r)) => Some(r),
                    Some(ID::Lane(l)) => Some(app.primary.map.get_l(l).parent),
                    _ => None,
                }
                .and_then(|r| {
                    if app.primary.map.get_r(r).is_light_rail() {
                        None
                    } else {
                        Some(ID::Road(r))
                    }
                });
        }
        if let Some(ID::Road(r)) = app.primary.current_selection {
            if ctx.normal_left_click() {
                return Transition::Multi(vec![
                    Transition::Push(RoadEditor::new_state_without_lane(ctx, app, r)),
                    Transition::Push(Warping::new_state(
                        ctx,
                        ctx.canvas.get_cursor_in_map_space().unwrap(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    )),
                ]);
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.network_layer);
            self.magnifying_glass.draw(g, app);
        }
        g.redraw(&self.edits_layer);
    }
}

fn make_top_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    let mut file_management = Vec::new();
    let edits = app.primary.map.get_edits();

    let total_mileage = {
        // Look for the new lanes...
        let mut total = Distance::ZERO;
        // TODO We're assuming the edits have been compressed.
        for cmd in &edits.commands {
            if let EditCmd::ChangeRoad { r, old, new } = cmd {
                let num_before = old
                    .lanes_ltr
                    .iter()
                    .filter(|spec| spec.lt == LaneType::Biking)
                    .count();
                let num_after = new
                    .lanes_ltr
                    .iter()
                    .filter(|spec| spec.lt == LaneType::Biking)
                    .count();
                if num_before != num_after {
                    let multiplier = (num_after as f64) - (num_before) as f64;
                    total += multiplier * app.primary.map.get_r(*r).center_pts.length();
                }
            }
        }
        total
    };
    if edits.commands.is_empty() {
        file_management.push("Today's network".text_widget(ctx));
    } else {
        file_management.push(Line(&edits.edits_name).into_widget(ctx));
    }
    file_management.push(
        Line(format!(
            "{:.1} miles of new bike lanes",
            total_mileage.to_miles()
        ))
        .secondary()
        .into_widget(ctx),
    );
    file_management.push(crate::ungap::legend(ctx, EDITED_COLOR, "changed road"));
    file_management.push(Widget::row(vec![
        ctx.style()
            .btn_outline
            .text("Open a proposal")
            .hotkey(lctrl(Key::O))
            .build_def(ctx),
        ctx.style()
            .btn_outline
            .text("Save this proposal")
            .hotkey(lctrl(Key::S))
            .disabled(edits.commands.is_empty())
            .build_def(ctx),
    ]));
    // TODO Should undo/redo, save, share functionality also live here?

    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            Line("Draw your ideal bike network")
                .small_heading()
                .into_widget(ctx),
            // TODO Or maybe this is misleading; we should keep the tab style
            ctx.style().btn_close_widget(ctx),
        ]),
        Widget::col(file_management).bg(ctx.style().section_bg),
        Widget::row(vec![
            "Click a road to edit in detail"
                .text_widget(ctx)
                .centered_vert(),
            ctx.style()
                .btn_solid_primary
                .text("Sketch a route")
                .hotkey(Key::S)
                .build_def(ctx),
        ])
        .evenly_spaced(),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

pub fn render_edits(ctx: &mut EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    let map = &app.primary.map;
    for r in &map.get_edits().changed_roads {
        batch.push(
            EDITED_COLOR.alpha(0.5),
            map.get_r(*r).get_thick_polygon(map),
        );
    }
    batch.upload(ctx)
}

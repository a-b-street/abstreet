use std::collections::{BTreeMap, BTreeSet};

use geom::Distance;
use map_model::{Block, Perimeter};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::ltn::Neighborhood;

const UNSELECTED: Color = Color::BLUE;
const SELECTED: Color = Color::CYAN;

pub struct SelectBoundary {
    panel: Panel,
    id_counter: usize,
    blocks: BTreeMap<Obj, Block>,
    world: World<Obj>,
    selected: BTreeSet<Obj>,
    draw_outline: Drawable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Obj(usize);
impl ObjectID for Obj {}

impl SelectBoundary {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        initial_boundary: Option<Perimeter>,
    ) -> Box<dyn State<App>> {
        let mut state = SelectBoundary {
            panel: make_panel(ctx, app, false),
            id_counter: 0,
            blocks: BTreeMap::new(),
            world: World::bounded(app.primary.map.get_bounds()),
            selected: BTreeSet::new(),
            draw_outline: Drawable::empty(ctx),
        };

        ctx.loading_screen("calculate all blocks", |ctx, timer| {
            timer.start("find single blocks");
            let perimeters = Perimeter::find_all_single_blocks(&app.primary.map);
            timer.stop("find single blocks");

            timer.start_iter("blockify", perimeters.len());
            for perimeter in perimeters {
                timer.next();
                match perimeter.to_block(&app.primary.map) {
                    Ok(block) => {
                        let id = state.new_id();
                        state.add_block(ctx, id, UNSELECTED, block);
                    }
                    Err(err) => {
                        warn!("Failed to make a block from a perimeter: {}", err);
                    }
                }
            }
        });

        if let Some(perimeter) = initial_boundary {
            let mut included = Vec::new();
            for (id, block) in &state.blocks {
                if perimeter.contains(&block.perimeter) {
                    included.push(*id);
                }
            }
            for id in included {
                state.selected.insert(id);
                state.block_changed(ctx, app, id);
            }
        }

        state.world.initialize_hover(ctx);
        Box::new(state)
    }

    fn new_id(&mut self) -> Obj {
        let id = Obj(self.id_counter);
        self.id_counter += 1;
        id
    }

    fn add_block(&mut self, ctx: &mut EventCtx, id: Obj, color: Color, block: Block) {
        let mut obj = self
            .world
            .add(id)
            .hitbox(block.polygon.clone())
            .draw_color(color.alpha(0.5))
            .hover_alpha(0.8)
            .clickable();
        if self.selected.contains(&id) {
            obj = obj.hotkey(Key::Space, "remove")
        } else {
            obj = obj.hotkey(Key::Space, "add")
        }
        obj.build(ctx);
        self.blocks.insert(id, block);
    }

    fn merge_selected(&self) -> Vec<Perimeter> {
        let mut perimeters = Vec::new();
        for id in &self.selected {
            perimeters.push(self.blocks[&id].perimeter.clone());
        }
        Perimeter::merge_all(perimeters, false)
    }

    fn block_changed(&mut self, ctx: &mut EventCtx, app: &App, id: Obj) {
        let block = self.blocks.remove(&id).unwrap();
        self.world.delete_before_replacement(id);
        self.add_block(
            ctx,
            id,
            if self.selected.contains(&id) {
                SELECTED
            } else {
                UNSELECTED
            },
            block,
        );

        // Draw the outline of the current blocks
        let mut valid_blocks = 0;
        let mut batch = GeomBatch::new();

        for perimeter in self.merge_selected() {
            if let Ok(block) = perimeter.to_block(&app.primary.map) {
                if let Ok(outline) = block.polygon.to_outline(Distance::meters(10.0)) {
                    // Alternate colors, to help people figure out where two disjoint boundaries
                    // exist
                    // TODO Ideally have more than 2 colors to cycle through
                    batch.push(
                        if valid_blocks % 2 == 0 {
                            Color::RED
                        } else {
                            Color::GREEN
                        },
                        outline,
                    );
                }
                valid_blocks += 1;
            }
        }
        self.draw_outline = batch.upload(ctx);
        self.panel = make_panel(ctx, app, valid_blocks == 1);
    }
}

impl State<App> for SelectBoundary {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Cancel" => {
                    return Transition::Pop;
                }
                "Confirm" => {
                    let mut perimeters = self.merge_selected();
                    assert_eq!(perimeters.len(), 1);
                    return Transition::Replace(super::viewer::Viewer::new_state(
                        ctx,
                        app,
                        Neighborhood::new(ctx, app, perimeters.pop().unwrap()),
                    ));
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add", id) => {
                self.selected.insert(id);
                self.block_changed(ctx, app, id);
            }
            WorldOutcome::Keypress("remove", id) => {
                self.selected.remove(&id);
                self.block_changed(ctx, app, id);
            }
            WorldOutcome::ClickedObject(id) => {
                if self.selected.contains(&id) {
                    self.selected.remove(&id);
                } else {
                    self.selected.insert(id);
                }
                self.block_changed(ctx, app, id);
            }
            _ => {}
        }
        // TODO Bypasses World...
        if ctx.redo_mouseover() {
            if let Some(id) = self.world.get_hovering() {
                if ctx.is_key_down(Key::LeftControl) {
                    if !self.selected.contains(&id) {
                        self.selected.insert(id);
                        self.block_changed(ctx, app, id);
                    }
                } else if ctx.is_key_down(Key::LeftShift) {
                    if self.selected.contains(&id) {
                        self.selected.remove(&id);
                        self.block_changed(ctx, app, id);
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.world.draw(g);
        g.redraw(&self.draw_outline);
        self.panel.draw(g);
    }
}

fn make_panel(ctx: &mut EventCtx, app: &App, boundary_ok: bool) -> Panel {
    Panel::new_builder(Widget::col(vec![
        map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
        "Draw a custom boundary for a neighborhood"
            .text_widget(ctx)
            .centered_vert(),
        "Click to add/remove a block".text_widget(ctx),
        "Hold LCtrl and paint blocks to add".text_widget(ctx),
        "Hold LShift and paint blocks to remove".text_widget(ctx),
        Widget::row(vec![
            ctx.style()
                .btn_solid_primary
                .text("Confirm")
                .disabled(!boundary_ok)
                .disabled_tooltip("You must select one contiguous boundary")
                .build_def(ctx),
            ctx.style()
                .btn_solid_destructive
                .text("Cancel")
                .build_def(ctx),
        ]),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .build(ctx)
}

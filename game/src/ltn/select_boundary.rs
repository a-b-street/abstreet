use std::collections::{BTreeMap, BTreeSet};

use geom::Distance;
use map_model::{Block, Perimeter};
use widgetry::mapspace::ToggleZoomed;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::ltn::partition::NeighborhoodID;
use crate::ltn::Neighborhood;

const SELECTED: Color = Color::CYAN;

pub struct SelectBoundary {
    panel: Panel,
    // These are always single, unmerged blocks
    blocks: BTreeMap<BlockID, Block>,
    world: World<BlockID>,
    selected: BTreeSet<BlockID>,
    draw_outline: ToggleZoomed,
    block_to_neighborhood: BTreeMap<BlockID, NeighborhoodID>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BlockID(usize);
impl ObjectID for BlockID {}

impl SelectBoundary {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        initial_boundary: Option<Perimeter>,
    ) -> Box<dyn State<App>> {
        let mut state = SelectBoundary {
            panel: make_panel(ctx, app, false),
            blocks: BTreeMap::new(),
            world: World::bounded(app.primary.map.get_bounds()),
            selected: BTreeSet::new(),
            draw_outline: ToggleZoomed::empty(ctx),
            block_to_neighborhood: BTreeMap::new(),
        };

        ctx.loading_screen("calculate all blocks", |ctx, timer| {
            timer.start("find single blocks");
            let perimeters = Perimeter::find_all_single_blocks(&app.primary.map);
            timer.stop("find single blocks");

            let mut blocks = Vec::new();
            timer.start_iter("blockify", perimeters.len());
            for perimeter in perimeters {
                timer.next();
                match perimeter.to_block(&app.primary.map) {
                    Ok(block) => {
                        blocks.push(block);
                    }
                    Err(err) => {
                        warn!("Failed to make a block from a perimeter: {}", err);
                    }
                }
            }

            for (idx, block) in blocks.into_iter().enumerate() {
                let id = BlockID(idx);
                let neighborhood = app.session.partitioning.neighborhood_containing(&block);
                state.block_to_neighborhood.insert(id, neighborhood);
                let color = app.session.partitioning.neighborhoods[&neighborhood].1;
                state.add_block(ctx, id, color, block);
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

    fn add_block(&mut self, ctx: &mut EventCtx, id: BlockID, color: Color, block: Block) {
        let mut obj = self
            .world
            .add(id)
            .hitbox(block.polygon.clone())
            .draw_color(color.alpha(0.5))
            .hover_alpha(0.8)
            .clickable();
        if self.selected.contains(&id) {
            obj = obj
                .hotkey(Key::Space, "remove")
                .hotkey(Key::LeftShift, "remove")
        } else {
            obj = obj
                .hotkey(Key::Space, "add")
                .hotkey(Key::LeftControl, "add")
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

    fn block_changed(&mut self, ctx: &mut EventCtx, app: &App, id: BlockID) {
        let block = self.blocks.remove(&id).unwrap();
        self.world.delete_before_replacement(id);
        self.add_block(
            ctx,
            id,
            if self.selected.contains(&id) {
                SELECTED
            } else {
                // Use the original color. This assumes the partitioning has been updated, of
                // course
                let neighborhood = self.block_to_neighborhood[&id];
                app.session.partitioning.neighborhoods[&neighborhood].1
            },
            block,
        );

        // Draw the outline of the current blocks
        let mut valid_blocks = 0;
        let mut batch = ToggleZoomed::builder();

        for perimeter in self.merge_selected() {
            if let Ok(block) = perimeter.to_block(&app.primary.map) {
                // Alternate colors, to help people figure out where two disjoint boundaries exist
                // TODO Ideally have more than 2 colors to cycle through
                let color = if valid_blocks % 2 == 0 {
                    Color::RED
                } else {
                    Color::GREEN
                };
                valid_blocks += 1;

                if let Ok(outline) = block.polygon.to_outline(Distance::meters(10.0)) {
                    batch.unzoomed.push(color, outline);
                }
                if let Ok(outline) = block.polygon.to_outline(Distance::meters(5.0)) {
                    batch.zoomed.push(color.alpha(0.5), outline);
                }
            }
        }
        self.draw_outline = batch.build(ctx);
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
                    // TODO Persist the partitioning
                    return Transition::Replace(super::connectivity::Viewer::new_state(
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
        self.draw_outline.draw(g);
        self.panel.draw(g);
    }
}

fn make_panel(ctx: &mut EventCtx, app: &App, boundary_ok: bool) -> Panel {
    Panel::new_builder(Widget::col(vec![
        map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
        "Draw a custom boundary for a neighborhood"
            .text_widget(ctx)
            .centered_vert(),
        Text::from_all(vec![
            Line("Click").fg(ctx.style().text_hotkey_color),
            Line(" to add/remove a block"),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Hold "),
            Line(Key::LeftControl.describe()).fg(ctx.style().text_hotkey_color),
            Line(" and paint over blocks to add"),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Hold "),
            Line(Key::LeftShift.describe()).fg(ctx.style().text_hotkey_color),
            Line(" and paint over blocks to remove"),
        ])
        .into_widget(ctx),
        Widget::row(vec![
            ctx.style()
                .btn_solid_primary
                .text("Confirm")
                .hotkey(Key::Enter)
                .disabled(!boundary_ok)
                .disabled_tooltip("You must select one contiguous boundary")
                .build_def(ctx),
            ctx.style()
                .btn_solid_destructive
                .text("Cancel")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ]),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .build(ctx)
}

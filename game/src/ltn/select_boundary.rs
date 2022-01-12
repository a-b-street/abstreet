use std::collections::{BTreeMap, BTreeSet};

use geom::Distance;
use map_model::{Block, Perimeter, RoadID};
use widgetry::mapspace::ToggleZoomed;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::ltn::NeighborhoodID;

const SELECTED: Color = Color::CYAN;

pub struct SelectBoundary {
    panel: Panel,
    id: NeighborhoodID,
    // These are always single, unmerged blocks. Thus, these blocks never change -- only their
    // color and assignment to a neighborhood.
    blocks: BTreeMap<BlockID, Block>,
    world: World<BlockID>,
    selected: BTreeSet<BlockID>,
    draw_outline: ToggleZoomed,
    block_to_neighborhood: BTreeMap<BlockID, NeighborhoodID>,
    frontier: BTreeSet<BlockID>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BlockID(usize);
impl ObjectID for BlockID {}

impl SelectBoundary {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighborhoodID) -> Box<dyn State<App>> {
        let initial_boundary = app.session.partitioning.neighborhoods[&id]
            .0
            .perimeter
            .clone();

        let mut state = SelectBoundary {
            panel: make_panel(ctx, app),
            id,
            blocks: BTreeMap::new(),
            world: World::bounded(app.primary.map.get_bounds()),
            selected: BTreeSet::new(),
            draw_outline: ToggleZoomed::empty(ctx),
            block_to_neighborhood: BTreeMap::new(),
            frontier: BTreeSet::new(),
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
                if let Some(neighborhood) = app.session.partitioning.neighborhood_containing(&block)
                {
                    state.block_to_neighborhood.insert(id, neighborhood);
                } else {
                    // TODO What happened?
                    error!(
                        "Block doesn't belong to any neighborhood?! {:?}",
                        block.perimeter
                    );
                }
                if initial_boundary.contains(&block.perimeter) {
                    state.selected.insert(id);
                }
                state.blocks.insert(id, block);
            }
            state.frontier = calculate_frontier(&initial_boundary, &state.blocks);

            // Fill out the world initially
            for id in state.blocks.keys().cloned().collect::<Vec<_>>() {
                state.add_block(ctx, app, id);
            }
        });

        state.world.initialize_hover(ctx);
        Box::new(state)
    }

    fn add_block(&mut self, ctx: &mut EventCtx, app: &App, id: BlockID) {
        let color = if self.selected.contains(&id) {
            SELECTED
        } else if let Some(neighborhood) = self.block_to_neighborhood.get(&id) {
            // Use the original color. This assumes the partitioning has been updated, of
            // course
            app.session.partitioning.neighborhoods[neighborhood].1
        } else {
            // TODO A broken case, block has no neighborhood
            Color::RED
        };

        if self.frontier.contains(&id) {
            let mut obj = self
                .world
                .add(id)
                .hitbox(self.blocks[&id].polygon.clone())
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
        } else {
            // If we can't immediately add/remove the block, fade it out and don't allow clicking
            // it
            self.world
                .add(id)
                .hitbox(self.blocks[&id].polygon.clone())
                .draw_color(color.alpha(0.3))
                .build(ctx);
        }
    }

    fn merge_selected(&self) -> Vec<Perimeter> {
        let mut perimeters = Vec::new();
        for id in &self.selected {
            perimeters.push(self.blocks[&id].perimeter.clone());
        }
        Perimeter::merge_all(perimeters, false)
    }

    // This block was in the previous frontier; its inclusion in self.selected has changed.
    fn block_changed(&mut self, ctx: &mut EventCtx, app: &App, id: BlockID) {
        let mut perimeters = self.merge_selected();
        if perimeters.len() != 1 {
            // We split the current neighborhood in two.
            // TODO Figure out how to handle this. For now, don't allow and revert
            if self.selected.contains(&id) {
                self.selected.remove(&id);
            } else {
                self.selected.insert(id);
            }
            let label =
                "Splitting this neighborhood in two is currently unsupported".text_widget(ctx);
            self.panel.replace(ctx, "warning", label);
            return;
        }

        let old_frontier = std::mem::take(&mut self.frontier);
        let new_perimeter = perimeters.pop().unwrap();
        self.frontier = calculate_frontier(&new_perimeter, &self.blocks);

        // Redraw all of the blocks that changed
        let mut changed_blocks: Vec<BlockID> = old_frontier
            .symmetric_difference(&self.frontier)
            .cloned()
            .collect();
        // And always the current block
        changed_blocks.push(id);
        for changed in changed_blocks {
            self.world.delete_before_replacement(changed);
            self.add_block(ctx, app, changed);
        }

        // Draw the outline of the current blocks
        let mut batch = ToggleZoomed::builder();
        if let Ok(block) = new_perimeter.to_block(&app.primary.map) {
            if let Ok(outline) = block.polygon.to_outline(Distance::meters(10.0)) {
                batch.unzoomed.push(Color::RED, outline);
            }
            if let Ok(outline) = block.polygon.to_outline(Distance::meters(5.0)) {
                batch.zoomed.push(Color::RED.alpha(0.5), outline);
            }
        }
        // TODO If this fails, maybe also revert
        self.draw_outline = batch.build(ctx);
        self.panel = make_panel(ctx, app);
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
                    // Persist the partitioning
                    if let Ok(block) = perimeters.pop().unwrap().to_block(&app.primary.map) {
                        // TODO May need to recalculate colors!
                        app.session
                            .partitioning
                            .neighborhoods
                            .get_mut(&self.id)
                            .unwrap()
                            .0 = block;
                        return Transition::Replace(super::connectivity::Viewer::new_state(
                            ctx, app, self.id,
                        ));
                    } else {
                        // TODO Is it possible we wound up with a broken block?
                    }
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add", id) => {
                self.selected.insert(id);
                self.block_changed(ctx, app, id)
            }
            WorldOutcome::Keypress("remove", id) => {
                self.selected.remove(&id);
                self.block_changed(ctx, app, id)
            }
            WorldOutcome::ClickedObject(id) => {
                if self.selected.contains(&id) {
                    self.selected.remove(&id);
                } else {
                    self.selected.insert(id);
                }
                self.block_changed(ctx, app, id)
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

fn make_panel(ctx: &mut EventCtx, app: &App) -> Panel {
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
                .build_def(ctx),
            ctx.style()
                .btn_solid_destructive
                .text("Cancel")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ]),
        Text::new().into_widget(ctx).named("warning"),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .build(ctx)
}

// Blocks on the "frontier" are adjacent to the perimeter, either just inside or outside.
fn calculate_frontier(perim: &Perimeter, blocks: &BTreeMap<BlockID, Block>) -> BTreeSet<BlockID> {
    let perim_roads: BTreeSet<RoadID> = perim.roads.iter().map(|id| id.road).collect();

    let mut frontier = BTreeSet::new();
    for (block_id, block) in blocks {
        for road_side_id in &block.perimeter.roads {
            // If the perimeter has this RoadSideID on the same side, we're just inside. If it has
            // the other side, just on the outside. Either way, on the frontier.
            if perim_roads.contains(&road_side_id.road) {
                frontier.insert(*block_id);
                break;
            }
        }
    }
    frontier
}

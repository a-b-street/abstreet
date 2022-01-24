use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use geom::Distance;
use map_model::{Block, Perimeter, RoadID, RoadSideID};
use widgetry::mapspace::ToggleZoomed;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::{App, NeighborhoodID, Partitioning, Transition};

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

    orig_partitioning: Partitioning,
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
            world: World::bounded(app.map.get_bounds()),
            selected: BTreeSet::new(),
            draw_outline: ToggleZoomed::empty(ctx),
            block_to_neighborhood: BTreeMap::new(),
            frontier: BTreeSet::new(),

            orig_partitioning: app.session.partitioning.clone(),
        };

        for (idx, block) in app.session.partitioning.single_blocks.iter().enumerate() {
            let id = BlockID(idx);
            if let Some(neighborhood) = app.session.partitioning.neighborhood_containing(block) {
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
            state.blocks.insert(id, block.clone());
        }
        state.frontier = calculate_frontier(&initial_boundary, &state.blocks);

        // Fill out the world initially
        for id in state.blocks.keys().cloned().collect::<Vec<_>>() {
            state.add_block(ctx, app, id);
        }

        state.redraw_outline(ctx, app, initial_boundary);
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

    fn redraw_outline(&mut self, ctx: &mut EventCtx, app: &App, perimeter: Perimeter) {
        // Draw the outline of the current blocks
        let mut batch = ToggleZoomed::builder();
        if let Ok(block) = perimeter.to_block(&app.map) {
            if let Ok(outline) = block.polygon.to_outline(Distance::meters(10.0)) {
                batch.unzoomed.push(Color::RED, outline);
            }
            if let Ok(outline) = block.polygon.to_outline(Distance::meters(5.0)) {
                batch.zoomed.push(Color::RED.alpha(0.5), outline);
            }
        }
        // TODO If this fails, maybe also revert
        self.draw_outline = batch.build(ctx);
    }

    // This block was in the previous frontier; its inclusion in self.selected has changed.
    fn block_changed(&mut self, ctx: &mut EventCtx, app: &mut App, id: BlockID) -> Transition {
        match self.try_block_changed(app, id) {
            Ok(Some(new_neighborhood)) => {
                app.session.partitioning.recalculate_coloring();
                return Transition::Replace(SelectBoundary::new_state(ctx, app, new_neighborhood));
            }
            Ok(None) => {
                let old_frontier = std::mem::take(&mut self.frontier);
                let new_perimeter = app.session.partitioning.neighborhoods[&self.id]
                    .0
                    .perimeter
                    .clone();
                self.frontier = calculate_frontier(&new_perimeter, &self.blocks);

                // Redraw all of the blocks that changed
                let mut changed_blocks: Vec<BlockID> = old_frontier
                    .symmetric_difference(&self.frontier)
                    .cloned()
                    .collect();
                // And always the current block
                changed_blocks.push(id);

                if app.session.partitioning.recalculate_coloring() {
                    // The coloring of neighborhoods changed; this could possibly have impact far
                    // away. Just redraw all blocks.
                    changed_blocks.clear();
                    changed_blocks.extend(self.blocks.keys().cloned());
                }

                for changed in changed_blocks {
                    self.world.delete_before_replacement(changed);
                    self.add_block(ctx, app, changed);
                }

                // TODO Pass in the Block
                self.redraw_outline(ctx, app, new_perimeter);
                self.panel = make_panel(ctx, app);
            }
            Err(err) => {
                if self.selected.contains(&id) {
                    self.selected.remove(&id);
                } else {
                    self.selected.insert(id);
                }
                let label = err.to_string().text_widget(ctx);
                self.panel.replace(ctx, "warning", label);
            }
        }

        Transition::Keep
    }

    fn make_merged_block(&self, app: &App, input: Vec<BlockID>) -> Result<Block> {
        let mut perimeters = Vec::new();
        for id in input {
            perimeters.push(self.blocks[&id].perimeter.clone());
        }
        let mut merged = Perimeter::merge_all(perimeters, false);
        if merged.len() != 1 {
            bail!(format!(
                "Splitting this neighborhood into {} pieces is currently unsupported",
                merged.len()
            ));
        }
        merged.pop().unwrap().to_block(&app.map)
    }

    // Ok(Some(x)) means the current neighborhood was destroyed, and the caller should switch to
    // focusing on a different neigbhorhood
    fn try_block_changed(&mut self, app: &mut App, id: BlockID) -> Result<Option<NeighborhoodID>> {
        if self.selected.contains(&id) {
            self.add_block_to_current(app, id)
        } else {
            self.remove_block_from_current(app, id)
        }
    }

    fn add_block_to_current(
        &mut self,
        app: &mut App,
        id: BlockID,
    ) -> Result<Option<NeighborhoodID>> {
        let old_owner = app
            .session
            .partitioning
            .neighborhood_containing(&self.blocks[&id])
            .unwrap();
        // Ignore the return value if the old neighborhood is deleted
        self.transfer_block(app, id, old_owner, self.id)?;
        Ok(None)
    }

    fn remove_block_from_current(
        &mut self,
        app: &mut App,
        id: BlockID,
    ) -> Result<Option<NeighborhoodID>> {
        // Find all RoadSideIDs in the block matching the current neighborhood perimeter. Look for
        // the first one that borders another neighborhood, and transfer the block there.
        // TODO This can get unintuitive -- if we remove a block bordering two other
        // neighborhoods, which one should we donate to?
        let current_perim_set: BTreeSet<RoadSideID> = app.session.partitioning.neighborhoods
            [&self.id]
            .0
            .perimeter
            .roads
            .iter()
            .cloned()
            .collect();
        for road_side in &self.blocks[&id].perimeter.roads {
            if !current_perim_set.contains(road_side) {
                continue;
            }
            // Is there another neighborhood that has the other side of this road on its perimeter?
            // TODO We could map road -> BlockID then use block_to_neighborhood
            let other_side = road_side.other_side();
            if let Some((new_owner, _)) = app
                .session
                .partitioning
                .neighborhoods
                .iter()
                .find(|(_, (block, _))| block.perimeter.roads.contains(&other_side))
            {
                let new_owner = *new_owner;
                return self.transfer_block(app, id, self.id, new_owner);
            }
        }

        // We didn't find any match, so we're jettisoning a block near the edge of the map (or a
        // buggy area missing blocks). Create a new neighborhood with just this block.
        let new_owner = app
            .session
            .partitioning
            .create_new_neighborhood(self.blocks[&id].clone());
        let result = self.transfer_block(app, id, self.id, new_owner);
        if result.is_err() {
            // Revert the change above!
            app.session.partitioning.remove_new_neighborhood(new_owner);
        }
        result
    }

    // This doesn't use self.selected; it's agnostic to what the current block is
    // TODO Move it to Partitioning
    fn transfer_block(
        &mut self,
        app: &mut App,
        id: BlockID,
        old_owner: NeighborhoodID,
        new_owner: NeighborhoodID,
    ) -> Result<Option<NeighborhoodID>> {
        assert_ne!(old_owner, new_owner);

        // Is the newly expanded neighborhood a valid perimeter?
        let new_owner_blocks: Vec<BlockID> = self
            .block_to_neighborhood
            .iter()
            .filter_map(|(block, neighborhood)| {
                if *neighborhood == new_owner || *block == id {
                    Some(*block)
                } else {
                    None
                }
            })
            .collect();
        let new_neighborhood_block = self.make_merged_block(app, new_owner_blocks)?;

        // Is the old neighborhood, minus this block, still valid?
        // TODO refactor Neighborhood to BlockIDs?
        let old_owner_blocks: Vec<BlockID> = self
            .block_to_neighborhood
            .iter()
            .filter_map(|(block, neighborhood)| {
                if *neighborhood == old_owner && *block != id {
                    Some(*block)
                } else {
                    None
                }
            })
            .collect();
        if old_owner_blocks.is_empty() {
            // We're deleting the old neighborhood!
            app.session
                .partitioning
                .neighborhoods
                .get_mut(&new_owner)
                .unwrap()
                .0 = new_neighborhood_block;
            app.session
                .partitioning
                .neighborhoods
                .remove(&old_owner)
                .unwrap();
            self.block_to_neighborhood.insert(id, new_owner);
            // Tell the caller to recreate this SelectBoundary state, switching to the neighborhood
            // we just donated to, since the old is now gone
            return Ok(Some(new_owner));
        }

        let old_neighborhood_block = self.make_merged_block(app, old_owner_blocks)?;
        // Great! Do the transfer.
        app.session
            .partitioning
            .neighborhoods
            .get_mut(&old_owner)
            .unwrap()
            .0 = old_neighborhood_block;
        app.session
            .partitioning
            .neighborhoods
            .get_mut(&new_owner)
            .unwrap()
            .0 = new_neighborhood_block;

        self.block_to_neighborhood.insert(id, new_owner);
        Ok(None)
    }
}

impl State<App> for SelectBoundary {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Cancel" => {
                    // TODO If we destroyed the current neighborhood, then we cancel, we'll pop
                    // back to a different neighborhood than we started with. And also the original
                    // partitioning will have been lost!!!
                    app.session.partitioning = self.orig_partitioning.clone();
                    return Transition::Replace(super::connectivity::Viewer::new_state(
                        ctx, app, self.id,
                    ));
                }
                "Confirm" => {
                    return Transition::Replace(super::connectivity::Viewer::new_state(
                        ctx, app, self.id,
                    ));
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add", id) => {
                self.selected.insert(id);
                return self.block_changed(ctx, app, id);
            }
            WorldOutcome::Keypress("remove", id) => {
                self.selected.remove(&id);
                return self.block_changed(ctx, app, id);
            }
            WorldOutcome::ClickedObject(id) => {
                if self.selected.contains(&id) {
                    self.selected.remove(&id);
                } else {
                    self.selected.insert(id);
                }
                return self.block_changed(ctx, app, id);
            }
            _ => {}
        }
        // TODO Bypasses World...
        if ctx.redo_mouseover() {
            if let Some(id) = self.world.get_hovering() {
                if ctx.is_key_down(Key::LeftControl) {
                    if !self.selected.contains(&id) {
                        self.selected.insert(id);
                        return self.block_changed(ctx, app, id);
                    }
                } else if ctx.is_key_down(Key::LeftShift) {
                    if self.selected.contains(&id) {
                        self.selected.remove(&id);
                        return self.block_changed(ctx, app, id);
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

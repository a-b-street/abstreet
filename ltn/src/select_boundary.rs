use std::collections::BTreeSet;

use anyhow::Result;

use geom::Distance;
use map_model::Block;
use widgetry::mapspace::ToggleZoomed;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::partition::BlockID;
use crate::{App, NeighborhoodID, Partitioning, Transition};

const SELECTED: Color = Color::CYAN;

pub struct SelectBoundary {
    panel: Panel,
    id: NeighborhoodID,
    world: World<BlockID>,
    // TODO Redundant
    selected: BTreeSet<BlockID>,
    draw_outline: ToggleZoomed,
    frontier: BTreeSet<BlockID>,

    orig_partitioning: Partitioning,
}

impl SelectBoundary {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighborhoodID) -> Box<dyn State<App>> {
        let initial_boundary = app.session.partitioning.neighborhood_block(id);

        let mut state = SelectBoundary {
            panel: make_panel(ctx, app),
            id,
            world: World::bounded(app.map.get_bounds()),
            selected: BTreeSet::new(),
            draw_outline: ToggleZoomed::empty(ctx),
            frontier: BTreeSet::new(),

            orig_partitioning: app.session.partitioning.clone(),
        };

        for (id, block) in app.session.partitioning.all_single_blocks() {
            if initial_boundary.perimeter.contains(&block.perimeter) {
                state.selected.insert(id);
            }
        }
        state.frontier = app
            .session
            .partitioning
            .calculate_frontier(&initial_boundary.perimeter);

        // Fill out the world initially
        for id in app.session.partitioning.all_block_ids() {
            state.add_block(ctx, app, id);
        }

        state.redraw_outline(ctx, initial_boundary);
        state.world.initialize_hover(ctx);
        Box::new(state)
    }

    fn add_block(&mut self, ctx: &mut EventCtx, app: &App, id: BlockID) {
        let color = if self.selected.contains(&id) {
            SELECTED
        } else {
            let neighborhood = app.session.partitioning.block_to_neighborhood(id);
            // Use the original color. This assumes the partitioning has been updated, of
            // course
            app.session.partitioning.neighborhood_color(neighborhood)
        };

        if self.frontier.contains(&id) {
            let mut obj = self
                .world
                .add(id)
                .hitbox(app.session.partitioning.get_block(id).polygon.clone())
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
                .hitbox(app.session.partitioning.get_block(id).polygon.clone())
                .draw_color(color.alpha(0.3))
                .build(ctx);
        }
    }

    fn redraw_outline(&mut self, ctx: &mut EventCtx, block: &Block) {
        // Draw the outline of the current blocks
        let mut batch = ToggleZoomed::builder();
        if let Ok(outline) = block.polygon.to_outline(Distance::meters(10.0)) {
            batch.unzoomed.push(Color::RED, outline);
        }
        if let Ok(outline) = block.polygon.to_outline(Distance::meters(5.0)) {
            batch.zoomed.push(Color::RED.alpha(0.5), outline);
        }
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
                self.frontier = app.session.partitioning.calculate_frontier(
                    &app.session
                        .partitioning
                        .neighborhood_block(self.id)
                        .perimeter,
                );

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
                    changed_blocks.extend(app.session.partitioning.all_block_ids());
                }

                for changed in changed_blocks {
                    self.world.delete_before_replacement(changed);
                    self.add_block(ctx, app, changed);
                }

                self.redraw_outline(ctx, app.session.partitioning.neighborhood_block(self.id));
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

    // Ok(Some(x)) means the current neighborhood was destroyed, and the caller should switch to
    // focusing on a different neigbhorhood
    fn try_block_changed(&mut self, app: &mut App, id: BlockID) -> Result<Option<NeighborhoodID>> {
        if self.selected.contains(&id) {
            let old_owner = app
                .session
                .partitioning
                .neighborhood_containing(id)
                .unwrap();
            // Ignore the return value if the old neighborhood is deleted
            app.session
                .partitioning
                .transfer_block(&app.map, id, old_owner, self.id)?;
            Ok(None)
        } else {
            app.session
                .partitioning
                .remove_block_from_neighborhood(&app.map, id, self.id)
        }
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

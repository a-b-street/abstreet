use std::collections::BTreeSet;

use anyhow::Result;

use geom::Distance;
use map_gui::tools::DrawRoadLabels;
use map_model::Block;
use widgetry::mapspace::ToggleZoomed;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::partition::BlockID;
use crate::{App, NeighborhoodID, Partitioning, Transition};

pub struct SelectBoundary {
    panel: Panel,
    id: NeighborhoodID,
    world: World<BlockID>,
    draw_outline: ToggleZoomed,
    frontier: BTreeSet<BlockID>,

    orig_partitioning: Partitioning,

    // As an optimization, don't repeatedly attempt to make an edit that'll fail. The bool is
    // whether the block is already included or not
    last_failed_change: Option<(BlockID, bool)>,

    labels: DrawRoadLabels,
}

impl SelectBoundary {
    pub fn new_state(ctx: &mut EventCtx, app: &App, id: NeighborhoodID) -> Box<dyn State<App>> {
        let mut state = SelectBoundary {
            panel: make_panel(ctx, app),
            id,
            world: World::bounded(app.map.get_bounds()),
            draw_outline: ToggleZoomed::empty(ctx),
            frontier: BTreeSet::new(),

            orig_partitioning: app.session.partitioning.clone(),
            last_failed_change: None,

            labels: DrawRoadLabels::only_major_roads(),
        };

        let initial_boundary = app.session.partitioning.neighborhood_block(id);
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
        let neighborhood = app.session.partitioning.block_to_neighborhood(id);
        let color = app.session.partitioning.neighborhood_color(neighborhood);

        if self.frontier.contains(&id) {
            let have_block = self.currently_have_block(app, id);
            let mut obj = self
                .world
                .add(id)
                .hitbox(app.session.partitioning.get_block(id).polygon.clone())
                .draw_color(color.alpha(0.5))
                .hover_alpha(0.8)
                .clickable();
            if have_block {
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
            let alpha = if self.id == neighborhood { 0.5 } else { 0.1 };
            self.world
                .add(id)
                .hitbox(app.session.partitioning.get_block(id).polygon.clone())
                .draw_color(color.alpha(alpha))
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

    // If the block is part of the current neighborhood, remove it. Otherwise add it. It's assumed
    // this block is in the previous frontier
    fn toggle_block(&mut self, ctx: &mut EventCtx, app: &mut App, id: BlockID) -> Transition {
        if self.last_failed_change == Some((id, self.currently_have_block(app, id))) {
            return Transition::Keep;
        }
        self.last_failed_change = None;

        match self.try_toggle_block(app, id) {
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
                self.last_failed_change = Some((id, self.currently_have_block(app, id)));
                let label = err.to_string().text_widget(ctx);
                self.panel.replace(ctx, "warning", label);
            }
        }

        Transition::Keep
    }

    // Ok(Some(x)) means the current neighborhood was destroyed, and the caller should switch to
    // focusing on a different neigbhorhood
    fn try_toggle_block(&mut self, app: &mut App, id: BlockID) -> Result<Option<NeighborhoodID>> {
        if self.currently_have_block(app, id) {
            app.session
                .partitioning
                .remove_block_from_neighborhood(&app.map, id, self.id)
        } else {
            let old_owner = app.session.partitioning.block_to_neighborhood(id);
            // Ignore the return value if the old neighborhood is deleted
            app.session
                .partitioning
                .transfer_block(&app.map, id, old_owner, self.id)?;
            Ok(None)
        }
    }

    fn currently_have_block(&self, app: &App, id: BlockID) -> bool {
        app.session.partitioning.block_to_neighborhood(id) == self.id
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
                    return Transition::Replace(crate::connectivity::Viewer::new_state(
                        ctx, app, self.id,
                    ));
                }
                "Confirm" => {
                    return Transition::Replace(crate::connectivity::Viewer::new_state(
                        ctx, app, self.id,
                    ));
                }
                x => {
                    return crate::handle_app_header_click(ctx, app, x).unwrap();
                }
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add" | "remove", id) | WorldOutcome::ClickedObject(id) => {
                return self.toggle_block(ctx, app, id);
            }
            _ => {}
        }
        // TODO Bypasses World...
        if ctx.redo_mouseover() {
            if let Some(id) = self.world.get_hovering() {
                if ctx.is_key_down(Key::LeftControl) {
                    if !self.currently_have_block(app, id) {
                        return self.toggle_block(ctx, app, id);
                    }
                } else if ctx.is_key_down(Key::LeftShift) {
                    if self.currently_have_block(app, id) {
                        return self.toggle_block(ctx, app, id);
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.world.draw(g);
        self.draw_outline.draw(g);
        self.panel.draw(g);
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }
}

fn make_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    Panel::new_builder(Widget::col(vec![
        crate::app_header(ctx, app),
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

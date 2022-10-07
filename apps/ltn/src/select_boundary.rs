use std::collections::BTreeSet;

use anyhow::Result;

use geom::Polygon;
use map_gui::tools::DrawSimpleRoadLabels;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::tools::{Lasso, PopupMsg};
use widgetry::{
    Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use crate::components::{AppwidePanel, Mode};
use crate::edit::EditMode;
use crate::partition::BlockID;
use crate::pick_area::draw_boundary_roads;
use crate::{colors, mut_partitioning, App, NeighbourhoodID, Partitioning, Transition};

pub struct SelectBoundary {
    appwide_panel: AppwidePanel,
    left_panel: Panel,
    id: NeighbourhoodID,
    world: World<BlockID>,
    draw_boundary_roads: Drawable,
    frontier: BTreeSet<BlockID>,

    orig_partitioning: Partitioning,

    // As an optimization, don't repeatedly attempt to make an edit that'll fail. The bool is
    // whether the block is already included or not
    last_failed_change: Option<(BlockID, bool)>,

    lasso: Option<Lasso>,
}

impl SelectBoundary {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        id: NeighbourhoodID,
    ) -> Box<dyn State<App>> {
        if app.partitioning().broken {
            return PopupMsg::new_state(
                ctx,
                "Error",
                vec![
                    "Sorry, you can't adjust any boundaries on this map.",
                    "This is a known problem without any workaround yet.",
                ],
            );
        }

        if app.per_map.draw_all_road_labels.is_none() {
            app.per_map.draw_all_road_labels = Some(DrawSimpleRoadLabels::all_roads(
                ctx,
                app,
                colors::ROAD_LABEL,
            ));
        }

        // Make sure we clear this state if we ever modify neighbourhood boundaries
        if let EditMode::Shortcuts(ref mut maybe_focus) = app.session.edit_mode {
            *maybe_focus = None;
        }
        if let EditMode::FreehandFilters(_) = app.session.edit_mode {
            app.session.edit_mode = EditMode::Filters;
        }

        let appwide_panel = AppwidePanel::new(ctx, app, Mode::SelectBoundary);
        let left_panel = make_panel(ctx, app, id, &appwide_panel.top_panel);
        let mut state = SelectBoundary {
            appwide_panel,
            left_panel,
            id,
            world: World::bounded(app.per_map.map.get_bounds()),
            draw_boundary_roads: draw_boundary_roads(ctx, app),
            frontier: BTreeSet::new(),

            orig_partitioning: app.partitioning().clone(),
            last_failed_change: None,

            lasso: None,
        };

        let initial_boundary = app.partitioning().neighbourhood_block(id);
        state.frontier = app
            .partitioning()
            .calculate_frontier(&initial_boundary.perimeter);

        // Fill out the world initially
        for id in app.partitioning().all_block_ids() {
            state.add_block(ctx, app, id);
        }

        state.world.initialize_hover(ctx);
        Box::new(state)
    }

    fn add_block(&mut self, ctx: &mut EventCtx, app: &App, id: BlockID) {
        if self.currently_have_block(app, id) {
            let mut obj = self
                .world
                .add(id)
                .hitbox(app.partitioning().get_block(id).polygon.clone())
                .draw_color(colors::BLOCK_IN_BOUNDARY)
                .hover_alpha(0.8);
            if self.frontier.contains(&id) {
                obj = obj
                    .hotkey(Key::Space, "remove")
                    .hotkey(Key::LeftShift, "remove")
                    .clickable();
            }
            obj.build(ctx);
        } else if self.frontier.contains(&id) {
            self.world
                .add(id)
                .hitbox(app.partitioning().get_block(id).polygon.clone())
                .draw_color(colors::BLOCK_IN_FRONTIER)
                .hover_alpha(0.8)
                .hotkey(Key::Space, "add")
                .hotkey(Key::LeftControl, "add")
                .clickable()
                .build(ctx);
        } else {
            // TODO Adds an invisible, non-clickable block. Don't add the block at all then?
            self.world
                .add(id)
                .hitbox(app.partitioning().get_block(id).polygon.clone())
                .draw(GeomBatch::new())
                .build(ctx);
        }
    }

    // If the block is part of the current neighbourhood, remove it. Otherwise add it. It's assumed
    // this block is in the previous frontier
    fn toggle_block(&mut self, ctx: &mut EventCtx, app: &mut App, id: BlockID) -> Transition {
        if self.last_failed_change == Some((id, self.currently_have_block(app, id))) {
            return Transition::Keep;
        }
        self.last_failed_change = None;

        match self.try_toggle_block(app, id) {
            Ok(Some(new_neighbourhood)) => {
                return Transition::Replace(SelectBoundary::new_state(ctx, app, new_neighbourhood));
            }
            Ok(None) => {
                let old_frontier = std::mem::take(&mut self.frontier);
                self.frontier = app
                    .partitioning()
                    .calculate_frontier(&app.partitioning().neighbourhood_block(self.id).perimeter);

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

                self.draw_boundary_roads = draw_boundary_roads(ctx, app);
                self.left_panel = make_panel(ctx, app, self.id, &self.appwide_panel.top_panel);
            }
            Err(err) => {
                self.last_failed_change = Some((id, self.currently_have_block(app, id)));
                let label = Text::from(Line(err.to_string()))
                    .wrap_to_pct(ctx, 15)
                    .into_widget(ctx);
                self.left_panel.replace(ctx, "warning", label);
            }
        }

        Transition::Keep
    }

    // Ok(Some(x)) means the current neighbourhood was destroyed, and the caller should switch to
    // focusing on a different neighborhood
    fn try_toggle_block(&mut self, app: &mut App, id: BlockID) -> Result<Option<NeighbourhoodID>> {
        if self.currently_have_block(app, id) {
            mut_partitioning!(app).remove_block_from_neighbourhood(&app.per_map.map, id, self.id)
        } else {
            let old_owner = app.partitioning().block_to_neighbourhood(id);
            // Ignore the return value if the old neighbourhood is deleted
            mut_partitioning!(app).transfer_block(&app.per_map.map, id, old_owner, self.id)?;
            Ok(None)
        }
    }

    fn currently_have_block(&self, app: &App, id: BlockID) -> bool {
        app.partitioning().block_to_neighbourhood(id) == self.id
    }

    fn add_blocks_freehand(&mut self, ctx: &mut EventCtx, app: &mut App, lasso_polygon: Polygon) {
        ctx.loading_screen("expand current neighbourhood boundary", |ctx, timer| {
            timer.start("find matching blocks");
            // Find all of the blocks within the polygon
            let mut add_blocks = Vec::new();
            for (id, block) in app.partitioning().all_single_blocks() {
                if lasso_polygon.contains_pt(block.polygon.center()) {
                    if app.partitioning().block_to_neighbourhood(id) != self.id {
                        add_blocks.push(id);
                    }
                }
            }
            timer.stop("find matching blocks");

            while !add_blocks.is_empty() {
                // Proceed in rounds. Calculate the current frontier, find all of the blocks in there,
                // try to add them, repeat.
                //
                // It should be safe to add multiple blocks in a round without recalculating the
                // frontier; adding one block shouldn't mess up the frontier for another
                let mut changed = false;
                let mut still_todo = Vec::new();
                timer.start_iter("try to add blocks", add_blocks.len());
                for block_id in add_blocks.drain(..) {
                    timer.next();
                    if self.frontier.contains(&block_id) {
                        let old_owner = app.partitioning().block_to_neighbourhood(block_id);
                        if let Ok(_) = mut_partitioning!(app).transfer_block(
                            &app.per_map.map,
                            block_id,
                            old_owner,
                            self.id,
                        ) {
                            changed = true;
                        } else {
                            still_todo.push(block_id);
                        }
                    } else {
                        still_todo.push(block_id);
                    }
                }
                if changed {
                    add_blocks = still_todo;
                    self.frontier = app.partitioning().calculate_frontier(
                        &app.partitioning().neighbourhood_block(self.id).perimeter,
                    );
                } else {
                    info!("Giving up on adding {} blocks", still_todo.len());
                    break;
                }
            }

            // Just redraw everything
            self.world = World::bounded(app.per_map.map.get_bounds());
            for id in app.partitioning().all_block_ids() {
                self.add_block(ctx, app, id);
            }
            self.draw_boundary_roads = draw_boundary_roads(ctx, app);
        });
    }
}

impl State<App> for SelectBoundary {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(ref mut lasso) = self.lasso {
            if let Some(polygon) = lasso.event(ctx) {
                self.lasso = None;
                self.add_blocks_freehand(ctx, app, polygon);
                self.left_panel = make_panel(ctx, app, self.id, &self.appwide_panel.top_panel);
            }
            return Transition::Keep;
        }

        // PreserveState doesn't matter, can't switch proposals in SelectBoundary anyway
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::Route, help)
        {
            return t;
        }
        if let Some(t) = app
            .session
            .layers
            .event(ctx, &app.cs, Mode::SelectBoundary, None)
        {
            return t;
        }
        if let Outcome::Clicked(x) = self.left_panel.event(ctx) {
            match x.as_ref() {
                "Cancel" => {
                    // TODO If we destroyed the current neighbourhood, then we cancel, we'll pop
                    // back to a different neighbourhood than we started with. And also the original
                    // partitioning will have been lost!!!
                    app.per_map.alt_proposals.partitioning = self.orig_partitioning.clone();
                    return Transition::Replace(crate::design_ltn::DesignLTN::new_state(
                        ctx, app, self.id,
                    ));
                }
                "Confirm" => {
                    return Transition::Replace(crate::design_ltn::DesignLTN::new_state(
                        ctx, app, self.id,
                    ));
                }
                "Select freehand" => {
                    self.lasso = Some(Lasso::new());
                    self.left_panel = make_panel_for_lasso(ctx, &self.appwide_panel.top_panel);
                }
                _ => unreachable!(),
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
        self.draw_boundary_roads.draw(g);
        self.appwide_panel.draw(g);
        self.left_panel.draw(g);
        app.session.layers.draw(g, app);
        app.per_map.draw_all_road_labels.as_ref().unwrap().draw(g);
        if let Some(ref lasso) = self.lasso {
            lasso.draw(g);
        }
    }
}

fn make_panel(ctx: &mut EventCtx, app: &App, id: NeighbourhoodID, top_panel: &Panel) -> Panel {
    crate::components::LeftPanel::builder(
        ctx,
        top_panel,
        Widget::col(vec![
            Line("Adjusting neighbourhood boundary")
                .small_heading()
                .into_widget(ctx),
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
            format!(
                "Neighbourhood area: {}",
                app.partitioning().neighbourhood_area_km2(id)
            )
            .text_widget(ctx),
            ctx.style()
                .btn_outline
                .icon_text("system/assets/tools/select.svg", "Select freehand")
                .hotkey(Key::F)
                .build_def(ctx),
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
            Widget::placeholder(ctx, "warning"),
        ]),
    )
    .build(ctx)
}

fn make_panel_for_lasso(ctx: &mut EventCtx, top_panel: &Panel) -> Panel {
    crate::components::LeftPanel::builder(
        ctx,
        top_panel,
        Widget::col(vec![
            "Draw a custom boundary for a neighbourhood"
                .text_widget(ctx)
                .centered_vert(),
            Text::from_all(vec![
                Line("Click and drag").fg(ctx.style().text_hotkey_color),
                Line(" to select the blocks to add to this neighbourhood"),
            ])
            .into_widget(ctx),
        ]),
    )
    .build(ctx)
}

fn help() -> Vec<&'static str> {
    vec![
        "You can grow or shrink the blue neighbourhood boundary here.",
        "Due to various known issues, it's not always possible to draw the boundary you want.",
        "",
        "The aqua blocks show where you can currently expand the boundary.",
        "Hint: There may be very small blocks near complex roads.",
        "Try the freehand tool to select them.",
    ]
}

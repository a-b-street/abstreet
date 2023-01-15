use std::collections::BTreeSet;

use anyhow::Result;

use geom::{Distance, Circle, Polygon};
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
        // Just simplify the polygon first! Aggressively!
        let input = lasso_polygon.simplify(50.0);
        info!(
            "Original polygon has {} points, simplified {}",
            lasso_polygon.get_outer_ring().points().len(),
            input.get_outer_ring().points().len()
        );

        self.draw_boundary_roads = ctx.upload(neighbourhood_from_nada(app, input));
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
                    mut_partitioning!(app) = self.orig_partitioning.clone();
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
                    self.lasso = Some(Lasso::new(Color::YELLOW, Distance::meters(1.0)));
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

use geo::Contains;
use geom::FindClosest;
use map_model::{CommonEndpoint, Map, Perimeter, RoadID, RoadSideID, SideOfRoad};
use widgetry::Color;

fn neighbourhood_from_nada(app: &App, input: Polygon) -> GeomBatch {
    let mut batch = GeomBatch::new();
    let map = &app.per_map.map;

    batch.push(Color::YELLOW.alpha(0.3), input.clone());

    // Find interior roads totally within the input lasso
    /*let lasso_geo: geo::Polygon = input.clone().into();
    let mut interior: BTreeSet<RoadID> = BTreeSet::new();
    for road in map.all_roads() {
        let center: geo::LineString = (&road.center_pts).into();
        if lasso_geo.contains(&center) {
            interior.insert(road.id);
            batch.push(Color::GREEN, road.get_thick_polygon());
        }
    }*/

    // Snap each corner to a driveable intersection
    let mut closest = FindClosest::new(map.get_bounds());
    for i in map.all_intersections() {
        if i.roads.iter().any(|r| map.get_r(*r).is_driveable()) {
            closest.add_polygon(i.id, &i.polygon);
        }
    }

    let threshold = Distance::meters(50.0);
    let mut intersection_waypoints = Vec::new();
    for corner in input.get_outer_ring().points() {
        // Only snap to intersections strictly inside the lasso. If we mix inside/outside, it gets
        // very confusing
        if let Some((id, snapped_pt, _)) = closest
            .all_close_pts(*corner, threshold)
            .into_iter()
            .filter(|(_, pt, _)| input.contains_pt(*pt))
            .min_by_key(|(_, _, dist)| *dist)
        {
            intersection_waypoints.push(id);

            // Visualize that snapping...
            /*if let Ok(line) = geom::Line::new(*corner, snapped_pt) {
                batch.push(
                    Color::BLUE.alpha(0.7),
                    line.make_polygons(Distance::meters(2.0)),
                );
            }*/
        }
    }
    if intersection_waypoints.is_empty() {
        return batch;
    }
    intersection_waypoints.push(intersection_waypoints[0]);

    // Now pathfind between each pair of waypoints
    let mut perim_roads = Vec::new();
    for pair in intersection_waypoints.windows(2) {
        if let Some((roads, _)) =
            map.simple_path_btwn_v2(pair[0], pair[1], map_model::PathConstraints::Car)
        {
            for r in roads {
                perim_roads.push(r);
            }
        }
    }
    // TODO Last to first?

    let trimmed_perim_roads = remove_spurs(perim_roads);
    for r in &trimmed_perim_roads {
        batch.push(Color::RED.alpha(0.6), map.get_r(*r).get_thick_polygon());
    }

    // Turn each one of these into a RoadSideID
    //
    // idea 1: just calculate a point halfway down the road's center line, projected left or right
    // accordingly. See which one is closer to the polylabel of the lasso.
    println!("Perim before fixing:");
    let mut road_sides = Vec::new();
    let lasso_center = input.polylabel();
    batch.push(
        Color::GREEN,
        Circle::new(lasso_center, Distance::meters(30.0)).to_polygon(),
    );
    for r in trimmed_perim_roads {
        let road = map.get_r(r);
        let (pt, angle) = road.center_pts.must_dist_along(road.length() / 2.0);
        let left_pt = pt.project_away(road.get_width() / 2.0, angle.rotate_degs(-90.0));
        let right_pt = pt.project_away(road.get_width() / 2.0, angle.rotate_degs(90.0));

        let side = if left_pt.dist_to(lasso_center) < right_pt.dist_to(lasso_center) {
            SideOfRoad::Left
        } else {
            SideOfRoad::Right
        };
        road_sides.push(RoadSideID {
            road: road.id,
            side,
        });
        println!("- {side:?} of {r}");
    }

    // This'll pick the correct side "most" of the time. But we can figure out when tracing the
    // road-sides will cross a road, and make corrections.
    let road_sides = fix_road_side_crosses(map, road_sides);

    for x in &road_sides {
        batch.push(
            Color::BLUE,
            x.get_outermost_lane(map)
                .lane_center_pts
                .make_polygons(Distance::meters(3.0)),
        );
    }

    // Did we get lucky?
    let perim = Perimeter {
        roads: road_sides,
        interior: BTreeSet::new(),
    };
    match perim.to_block(map) {
        Ok(block) => {
            batch.push(Color::CYAN.alpha(0.5), block.polygon);
        }
        Err(err) => {
            println!("Failed: {err}");
        }
    }

    batch
}

fn remove_spurs(mut input: Vec<RoadID>) -> Vec<RoadID> {
    // If the input is a loop, then this'll cause problems
    if input.last() == Some(&input[0]) {
        input.pop();
    }

    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for r in input {
        if seen.contains(&r) {
            // Backtrack on the output until we find this road
            while output.last() != Some(&r) {
                seen.remove(&output.pop().unwrap());
            }
            assert_eq!(Some(r), output.pop());
            seen.remove(&r);
        } else {
            seen.insert(r);
            output.push(r);
        }
    }
    output

    // Just eliminate little "spurs" that're one road long
    /*let mut output = Vec::new();
    for r in perim_roads {
        if output.last() == Some(&r) {
            output.pop();
        } else {
            output.push(r);
        }
    }
    output*/
}

// Blindlyish assume the place we start is "correct" and match that
fn fix_road_side_crosses(map: &Map, mut input: Vec<RoadSideID>) -> Vec<RoadSideID> {
    if input.is_empty() {
        return input;
    }
    // No windows_mut, so index manually
    for idx in 0..input.len() - 1 {
        let lane1 = input[idx].get_outermost_lane(map);
        let lane2 = input[idx + 1].get_outermost_lane(map);
        let i = if let CommonEndpoint::One(i) = lane1.common_endpoint(lane2) {
            i
        } else {
            // ?? Just give up here
            continue;
        };
        // Trim back from the ends a bit
        // ... but then false positives everywhere
        let pt1 = if lane1.src_i == i {
            //lane1.lane_center_pts.percent_along(0.2).unwrap().0
            lane1.lane_center_pts.first_pt()
        } else {
            //lane1.lane_center_pts.percent_along(0.8).unwrap().0
            lane1.lane_center_pts.last_pt()
        };
        let pt2 = if lane2.src_i == i {
            //lane2.lane_center_pts.percent_along(0.2).unwrap().0
            lane2.lane_center_pts.first_pt()
        } else {
            //lane2.lane_center_pts.percent_along(0.8).unwrap().0
            lane2.lane_center_pts.last_pt()
        };

        // If the points are the same, we definitely don't cross the road, so cool!
        if let Ok(line) = geom::Line::new(pt1, pt2) {
            if map
                .get_r(input[idx + 1].road)
                .center_pts
                .intersection(&line.to_polyline())
                .is_some()
            {
                // Blindly fix!
                let id = input.get_mut(idx + 1).unwrap();
                println!("Swap {id:?}");
                *id = id.other_side();
            }
        }
    }
    input
}

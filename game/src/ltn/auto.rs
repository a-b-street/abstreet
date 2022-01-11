//! Experiments to make a neighborhood be low-traffic by automatically placing filters to prevent all rat runs.

use abstutil::Timer;
use widgetry::EventCtx;

use super::rat_runs::find_rat_runs;
use super::Neighborhood;
use crate::app::App;

/// Find the road with the most rat-runs that can be closed without creating a disconnected cell,
/// and filter it. There's a vague intuition or observation that the "bottleneck" will have the
/// most rat-runs going through it, so tackle the worst problem first.
pub fn greedy_heuristic(
    ctx: &EventCtx,
    app: &mut App,
    neighborhood: &Neighborhood,
    timer: &mut Timer,
) {
    if neighborhood
        .cells
        .iter()
        .filter(|c| c.is_disconnected())
        .count()
        != 0
    {
        warn!("Not applying the greedy heuristic to a neighborhood; it already has a disconnected cell");
        return;
    }

    let rat_runs = find_rat_runs(
        &app.primary.map,
        &neighborhood,
        &app.session.modal_filters,
        timer,
    );
    // TODO How should we break ties? Some rat-runs are worse than others; use that weight?
    // TODO Should this operation be per cell instead? We could hover on a road belonging to that
    // cell to select it
    if let Some((r, _)) = rat_runs
        .count_per_road
        .borrow()
        .iter()
        .max_by_key(|pair| pair.1)
    {
        let road = app.primary.map.get_r(*r);
        app.session
            .modal_filters
            .roads
            .insert(road.id, road.length() / 2.0);
        let new_neighborhood = Neighborhood::new(ctx, app, neighborhood.orig_perimeter.clone());
        if new_neighborhood
            .cells
            .iter()
            .filter(|c| c.is_disconnected())
            .count()
            != 0
        {
            warn!("Filtering {} disconnects a cell, never mind", road.id);
            app.session.modal_filters.roads.remove(&road.id).unwrap();
            // TODO Try the next choice
        }
    }
}

// TODO The brute force approach: try to filter every possible road, find the one with the least
// rat-runs by the end

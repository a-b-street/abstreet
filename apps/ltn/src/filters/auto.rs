//! Experiments to make a neighborhood be low-traffic by automatically placing filters to prevent all rat runs.

use anyhow::Result;

use abstutil::Timer;
use map_model::RoadID;
use widgetry::{Choice, EventCtx};

use crate::rat_runs::find_rat_runs;
use crate::{after_edit, App, Neighborhood};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heuristic {
    /// Find the road with the most rat-runs that can be closed without creating a disconnected
    /// cell, and filter it.
    ///
    /// There's a vague intuition or observation that the "bottleneck" will have the most rat-runs
    /// going through it, so tackle the worst problem first.
    Greedy,
    /// Try adding one filter to every possible road, counting the rat-runs after. Choose the next
    /// step by the least resulting rat runs.
    BruteForce,
    /// Find one filter that splits a cell, maximizing the number of streets in each new cell.
    SplitCells,
    /// Per cell, close all borders except for one. This doesn't affect connectivity, but prevents
    /// all rat-runs.
    OnlyOneBorder,
}

impl Heuristic {
    pub fn choices() -> Vec<Choice<Heuristic>> {
        vec![
            Choice::new(
                "filter the road with the most rat-runs (greedy)",
                Heuristic::Greedy,
            ),
            Choice::new(
                "stop the most rat-runs (brute-force)",
                Heuristic::BruteForce,
            ),
            Choice::new("split large cells", Heuristic::SplitCells),
            Choice::new("only one entrance per cell", Heuristic::OnlyOneBorder),
        ]
    }

    pub fn apply(
        self,
        ctx: &EventCtx,
        app: &mut App,
        neighborhood: &Neighborhood,
        timer: &mut Timer,
    ) -> Result<()> {
        if neighborhood
            .cells
            .iter()
            .filter(|c| c.is_disconnected())
            .count()
            != 0
        {
            bail!("This neighborhood has a disconnected cell; fix that first");
        }

        // TODO If we already have no rat-runs, stop

        app.session.modal_filters.before_edit();

        match self {
            Heuristic::Greedy => greedy(ctx, app, neighborhood, timer),
            Heuristic::BruteForce => brute_force(ctx, app, neighborhood, timer),
            Heuristic::SplitCells => split_cells(ctx, app, neighborhood, timer),
            Heuristic::OnlyOneBorder => only_one_border(app, neighborhood),
        }

        let empty = app.session.modal_filters.cancel_empty_edit();
        after_edit(ctx, app);
        if empty {
            bail!("No new filters created");
        } else {
            Ok(())
        }
    }
}

fn greedy(ctx: &EventCtx, app: &mut App, neighborhood: &Neighborhood, timer: &mut Timer) {
    let rat_runs = find_rat_runs(app, &neighborhood, timer);
    // TODO How should we break ties? Some rat-runs are worse than others; use that weight?
    // TODO Should this operation be per cell instead? We could hover on a road belonging to that
    // cell to select it
    if let Some((r, _)) = rat_runs
        .count_per_road
        .borrow()
        .iter()
        .max_by_key(|pair| pair.1)
    {
        if try_to_filter_road(ctx, app, neighborhood, *r).is_none() {
            warn!("Filtering {} disconnects a cell, never mind", r);
            // TODO Try the next choice
        }
    }
}

fn brute_force(ctx: &EventCtx, app: &mut App, neighborhood: &Neighborhood, timer: &mut Timer) {
    // Which road leads to the fewest rat-runs?
    let mut best: Option<(RoadID, usize)> = None;

    let orig_filters = app.session.modal_filters.roads.len();
    timer.start_iter(
        "evaluate candidate filters",
        neighborhood.orig_perimeter.interior.len(),
    );
    for r in &neighborhood.orig_perimeter.interior {
        timer.next();
        if app.session.modal_filters.roads.contains_key(r) {
            continue;
        }
        if let Some(new) = try_to_filter_road(ctx, app, neighborhood, *r) {
            let num_rat_runs =
                // This spams too many logs, and can't be used within a start_iter anyway
                find_rat_runs(app, &new, &mut Timer::throwaway())
                    .paths
                    .len();
            // TODO Again, break ties. Just the number of paths is kind of a weak metric.
            if best.map(|(_, score)| num_rat_runs < score).unwrap_or(true) {
                best = Some((*r, num_rat_runs));
            }
            // Always undo the new filter between each test
            app.session.modal_filters.roads.remove(r).unwrap();
        }

        assert_eq!(orig_filters, app.session.modal_filters.roads.len());
    }

    if let Some((r, _)) = best {
        try_to_filter_road(ctx, app, neighborhood, r).unwrap();
    }
}

fn split_cells(ctx: &EventCtx, app: &mut App, neighborhood: &Neighborhood, timer: &mut Timer) {
    // Filtering which road leads to new cells with the MOST streets in the smaller cell?
    let mut best: Option<(RoadID, usize)> = None;

    let orig_filters = app.session.modal_filters.roads.len();
    timer.start_iter(
        "evaluate candidate filters",
        neighborhood.orig_perimeter.interior.len(),
    );
    for r in &neighborhood.orig_perimeter.interior {
        timer.next();
        if app.session.modal_filters.roads.contains_key(r) {
            continue;
        }
        if let Some(new) = try_to_filter_road(ctx, app, neighborhood, *r) {
            // Did we split the cell?
            if new.cells.len() > neighborhood.cells.len() {
                // Find the two new cells
                let split_cells: Vec<_> = new
                    .cells
                    .iter()
                    .filter(|cell| cell.roads.contains_key(r))
                    .collect();
                assert_eq!(2, split_cells.len());
                // We want cells to be roughly evenly-sized. Just count the number of road segments
                // as a proxy for that.
                let new_score = split_cells[0].roads.len().min(split_cells[1].roads.len());
                if best
                    .map(|(_, old_score)| new_score > old_score)
                    .unwrap_or(true)
                {
                    best = Some((*r, new_score));
                }
            }
            // Always undo the new filter between each test
            app.session.modal_filters.roads.remove(r).unwrap();
        }

        assert_eq!(orig_filters, app.session.modal_filters.roads.len());
    }

    if let Some((r, _)) = best {
        try_to_filter_road(ctx, app, neighborhood, r).unwrap();
    }
}

fn only_one_border(app: &mut App, neighborhood: &Neighborhood) {
    for cell in &neighborhood.cells {
        if cell.borders.len() > 1 {
            // TODO How to pick which one to leave open?
            for i in cell.borders.iter().skip(1) {
                // Find the road in this cell connected to this border
                for r in cell.roads.keys() {
                    let road = app.map.get_r(*r);
                    if road.src_i == *i {
                        app.session
                            .modal_filters
                            .roads
                            .insert(road.id, 0.1 * road.length());
                        break;
                    } else if road.dst_i == *i {
                        app.session
                            .modal_filters
                            .roads
                            .insert(road.id, 0.9 * road.length());
                        break;
                    }
                }
            }
        }
    }
}

// If successful, returns a Neighborhood and leaves the new filter in place. If it disconncts a
// cell, reverts the change and returns None
fn try_to_filter_road(
    ctx: &EventCtx,
    app: &mut App,
    neighborhood: &Neighborhood,
    r: RoadID,
) -> Option<Neighborhood> {
    let road = app.map.get_r(r);
    app.session
        .modal_filters
        .roads
        .insert(r, road.length() / 2.0);
    // TODO This is expensive; can we just do the connectivity work and not drawing?
    let new_neighborhood = Neighborhood::new(ctx, app, neighborhood.id);
    if new_neighborhood.cells.iter().any(|c| c.is_disconnected()) {
        app.session.modal_filters.roads.remove(&r).unwrap();
        None
    } else {
        Some(new_neighborhood)
    }
}

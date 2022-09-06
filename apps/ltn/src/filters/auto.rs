//! Experiments to make a neighbourhood be low-traffic by automatically placing filters to prevent
//! all shortcuts.

use anyhow::Result;

use abstutil::Timer;
use map_model::RoadID;
use widgetry::{Choice, EventCtx};

use crate::{after_edit, App, Neighbourhood, RoadFilter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heuristic {
    /// Find the road with the most shortcuts that can be closed without creating a disconnected
    /// cell, and filter it.
    ///
    /// There's a vague intuition or observation that the "bottleneck" will have the most shortcuts
    /// going through it, so tackle the worst problem first.
    Greedy,
    /// Try adding one filter to every possible road, counting the shortcuts after. Choose the next
    /// step by the least resulting shortcuts.
    BruteForce,
    /// Find one filter that splits a cell, maximizing the number of streets in each new cell.
    SplitCells,
    /// Per cell, close all borders except for one. This doesn't affect connectivity, but prevents
    /// all shortcuts.
    OnlyOneBorder,
}

impl Heuristic {
    pub fn choices() -> Vec<Choice<Heuristic>> {
        vec![
            Choice::new(
                "filter the road with the most shortcuts (greedy)",
                Heuristic::Greedy,
            ),
            Choice::new(
                "stop the most shortcuts (brute-force)",
                Heuristic::BruteForce,
            ),
            Choice::new("split large cells", Heuristic::SplitCells),
            Choice::new("only one entrance per cell", Heuristic::OnlyOneBorder),
        ]
    }

    pub fn apply(
        self,
        ctx: &mut EventCtx,
        app: &mut App,
        neighbourhood: &Neighbourhood,
        timer: &mut Timer,
    ) -> Result<()> {
        if neighbourhood
            .cells
            .iter()
            .filter(|c| c.is_disconnected())
            .count()
            != 0
        {
            bail!("This neighbourhood has a disconnected cell; fix that first");
        }

        // TODO If we already have no shortcuts, stop

        app.per_map.edits.before_edit();

        match self {
            Heuristic::Greedy => greedy(ctx, app, neighbourhood),
            Heuristic::BruteForce => brute_force(ctx, app, neighbourhood, timer),
            Heuristic::SplitCells => split_cells(ctx, app, neighbourhood, timer),
            Heuristic::OnlyOneBorder => only_one_border(app, neighbourhood),
        }

        let empty = app.per_map.edits.cancel_empty_edit();
        after_edit(ctx, app);
        if empty {
            bail!("No new filters created");
        } else {
            Ok(())
        }
    }
}

fn greedy(ctx: &mut EventCtx, app: &mut App, neighbourhood: &Neighbourhood) {
    // TODO How should we break ties? Some shortcuts are worse than others; use that weight?
    // TODO Should this operation be per cell instead? We could hover on a road belonging to that
    // cell to select it
    if let Some((r, _)) = neighbourhood
        .shortcuts
        .count_per_road
        .borrow()
        .iter()
        .max_by_key(|pair| pair.1)
    {
        if try_to_filter_road(ctx, app, neighbourhood, *r).is_none() {
            warn!("Filtering {} disconnects a cell, never mind", r);
            // TODO Try the next choice
        }
    }
}

fn brute_force(
    ctx: &mut EventCtx,
    app: &mut App,
    neighbourhood: &Neighbourhood,
    timer: &mut Timer,
) {
    // Which road leads to the fewest shortcuts?
    let mut best: Option<(RoadID, usize)> = None;

    let orig_filters = app.per_map.edits.roads.len();
    timer.start_iter(
        "evaluate candidate filters",
        neighbourhood.orig_perimeter.interior.len(),
    );
    for r in &neighbourhood.orig_perimeter.interior {
        timer.next();
        if app.per_map.edits.roads.contains_key(r) {
            continue;
        }
        if let Some(new) = try_to_filter_road(ctx, app, neighbourhood, *r) {
            let num_shortcuts = new.shortcuts.paths.len();
            // TODO Again, break ties. Just the number of paths is kind of a weak metric.
            if best.map(|(_, score)| num_shortcuts < score).unwrap_or(true) {
                best = Some((*r, num_shortcuts));
            }
            // Always undo the new filter between each test
            app.per_map.edits.roads.remove(r).unwrap();
        }

        assert_eq!(orig_filters, app.per_map.edits.roads.len());
    }

    if let Some((r, _)) = best {
        try_to_filter_road(ctx, app, neighbourhood, r).unwrap();
    }
}

fn split_cells(
    ctx: &mut EventCtx,
    app: &mut App,
    neighbourhood: &Neighbourhood,
    timer: &mut Timer,
) {
    // Filtering which road leads to new cells with the MOST streets in the smaller cell?
    let mut best: Option<(RoadID, usize)> = None;

    let orig_filters = app.per_map.edits.roads.len();
    timer.start_iter(
        "evaluate candidate filters",
        neighbourhood.orig_perimeter.interior.len(),
    );
    for r in &neighbourhood.orig_perimeter.interior {
        timer.next();
        if app.per_map.edits.roads.contains_key(r) {
            continue;
        }
        if let Some(new) = try_to_filter_road(ctx, app, neighbourhood, *r) {
            // Did we split the cell?
            if new.cells.len() > neighbourhood.cells.len() {
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
            app.per_map.edits.roads.remove(r).unwrap();
        }

        assert_eq!(orig_filters, app.per_map.edits.roads.len());
    }

    if let Some((r, _)) = best {
        try_to_filter_road(ctx, app, neighbourhood, r).unwrap();
    }
}

fn only_one_border(app: &mut App, neighbourhood: &Neighbourhood) {
    for cell in &neighbourhood.cells {
        if cell.borders.len() > 1 {
            // TODO How to pick which one to leave open?
            for i in cell.borders.iter().skip(1) {
                // Find the road in this cell connected to this border
                for r in cell.roads.keys() {
                    let road = app.per_map.map.get_r(*r);
                    if road.src_i == *i {
                        app.per_map.edits.roads.insert(
                            road.id,
                            RoadFilter::new_by_user(0.1 * road.length(), app.session.filter_type),
                        );
                        break;
                    } else if road.dst_i == *i {
                        app.per_map.edits.roads.insert(
                            road.id,
                            RoadFilter::new_by_user(0.9 * road.length(), app.session.filter_type),
                        );
                        break;
                    }
                }
            }
        }
    }
}

// If successful, returns a Neighbourhood and leaves the new filter in place. If it disconncts a
// cell, reverts the change and returns None
fn try_to_filter_road(
    ctx: &mut EventCtx,
    app: &mut App,
    neighbourhood: &Neighbourhood,
    r: RoadID,
) -> Option<Neighbourhood> {
    let road = app.per_map.map.get_r(r);
    app.per_map.edits.roads.insert(
        r,
        RoadFilter::new_by_user(road.length() / 2.0, app.session.filter_type),
    );
    // TODO This is expensive; can we just do the connectivity work and not drawing?
    let new_neighbourhood = Neighbourhood::new(ctx, app, neighbourhood.id);
    if new_neighbourhood.cells.iter().any(|c| c.is_disconnected()) {
        app.per_map.edits.roads.remove(&r).unwrap();
        None
    } else {
        Some(new_neighbourhood)
    }
}

//! Experiments to make a neighbourhood be low-traffic by automatically placing filters to prevent
//! all shortcuts.

use anyhow::Result;

use abstutil::Timer;
use map_model::RoadFilter;
use map_model::RoadID;
use widgetry::{Choice, EventCtx};

use crate::{redraw_all_icons, App, Neighbourhood};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AutoFilterHeuristic {
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

impl AutoFilterHeuristic {
    pub fn choices() -> Vec<Choice<Self>> {
        vec![
            Choice::new(
                "filter the road with the most shortcuts (greedy)",
                AutoFilterHeuristic::Greedy,
            ),
            Choice::new(
                "stop the most shortcuts (brute-force)",
                AutoFilterHeuristic::BruteForce,
            ),
            Choice::new("split large cells", AutoFilterHeuristic::SplitCells),
            Choice::new(
                "only one entrance per cell",
                AutoFilterHeuristic::OnlyOneBorder,
            ),
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

        match self {
            AutoFilterHeuristic::Greedy => greedy(app, neighbourhood),
            AutoFilterHeuristic::BruteForce => brute_force(app, neighbourhood, timer),
            AutoFilterHeuristic::SplitCells => split_cells(app, neighbourhood, timer),
            AutoFilterHeuristic::OnlyOneBorder => only_one_border(app, neighbourhood),
        }

        // TODO Detect if we changed anything
        let empty = false;
        redraw_all_icons(ctx, app);
        if empty {
            bail!("No new filters created");
        } else {
            Ok(())
        }
    }
}

fn greedy(app: &mut App, neighbourhood: &Neighbourhood) {
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
        if try_to_filter_road(app, neighbourhood, *r).is_none() {
            warn!("Filtering {} disconnects a cell, never mind", r);
            // TODO Try the next choice
        }
    }
}

fn brute_force(app: &mut App, neighbourhood: &Neighbourhood, timer: &mut Timer) {
    // Which road leads to the fewest shortcuts?
    let mut best: Option<(RoadID, usize)> = None;

    let orig_filters = app.per_map.map.all_roads_with_modal_filter().count();
    timer.start_iter(
        "evaluate candidate filters",
        neighbourhood.interior_roads.len(),
    );
    for r in &neighbourhood.interior_roads {
        timer.next();
        if app.per_map.map.get_r(*r).modal_filter.is_some() {
            continue;
        }
        if let Some(new) = try_to_filter_road(app, neighbourhood, *r) {
            let num_shortcuts = new.shortcuts.paths.len();
            // TODO Again, break ties. Just the number of paths is kind of a weak metric.
            if best.map(|(_, score)| num_shortcuts < score).unwrap_or(true) {
                best = Some((*r, num_shortcuts));
            }
            // Always undo the new filter between each test
            remove_filter(app, *r);
        }

        assert_eq!(
            orig_filters,
            app.per_map.map.all_roads_with_modal_filter().count()
        );
    }

    if let Some((r, _)) = best {
        try_to_filter_road(app, neighbourhood, r).unwrap();
    }
}

fn split_cells(app: &mut App, neighbourhood: &Neighbourhood, timer: &mut Timer) {
    // Filtering which road leads to new cells with the MOST streets in the smaller cell?
    let mut best: Option<(RoadID, usize)> = None;

    let orig_filters = app.per_map.map.all_roads_with_modal_filter().count();
    timer.start_iter(
        "evaluate candidate filters",
        neighbourhood.interior_roads.len(),
    );
    for r in &neighbourhood.interior_roads {
        timer.next();
        if app.per_map.map.get_r(*r).modal_filter.is_some() {
            continue;
        }
        if let Some(new) = try_to_filter_road(app, neighbourhood, *r) {
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
            remove_filter(app, *r);
        }

        assert_eq!(
            orig_filters,
            app.per_map.map.all_roads_with_modal_filter().count()
        );
    }

    if let Some((r, _)) = best {
        try_to_filter_road(app, neighbourhood, r).unwrap();
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
                        add_filter(app, *r, 0.1);
                        break;
                    } else if road.dst_i == *i {
                        add_filter(app, *r, 0.9);
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
    app: &mut App,
    neighbourhood: &Neighbourhood,
    r: RoadID,
) -> Option<Neighbourhood> {
    add_filter(app, r, 0.5);
    let new_neighbourhood = Neighbourhood::new(app, neighbourhood.id);
    if new_neighbourhood.cells.iter().any(|c| c.is_disconnected()) {
        remove_filter(app, r);
        None
    } else {
        Some(new_neighbourhood)
    }
}

fn add_filter(app: &mut App, r: RoadID, pct: f64) {
    let map = &app.per_map.map;
    let mut edits = map.get_edits().clone();
    let road = map.get_r(r);
    edits.commands.push(map.edit_road_cmd(r, |new| {
        new.modal_filter = Some(RoadFilter::new(
            pct * road.length(),
            app.session.filter_type,
        ));
    }));
    app.apply_edits(edits);
}

fn remove_filter(app: &mut App, r: RoadID) {
    let map = &app.per_map.map;
    let mut edits = map.get_edits().clone();
    edits.commands.push(map.edit_road_cmd(r, |new| {
        new.modal_filter = None;
    }));
    app.apply_edits(edits);
}

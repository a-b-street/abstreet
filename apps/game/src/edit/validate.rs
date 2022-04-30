use std::collections::BTreeSet;

use abstutil::Timer;
use map_model::{connectivity, Direction, DrivingSide, EditCmd, Map, PathConstraints};
use widgetry::tools::PopupMsg;
use widgetry::{EventCtx, State};

use crate::app::App;

// Some of these take a candidate EditCmd to do, then see if it's valid. If they return None, it's
// fine. They always leave the map in the original state without the new EditCmd.

// Could be caused by closing intersections
pub fn check_sidewalk_connectivity(
    ctx: &mut EventCtx,
    app: &mut App,
    cmd: EditCmd,
) -> Option<Box<dyn State<App>>> {
    let orig_edits = app.primary.map.get_edits().clone();
    let (_, disconnected_before) =
        connectivity::find_scc(&app.primary.map, PathConstraints::Pedestrian);

    let mut edits = orig_edits.clone();
    edits.commands.push(cmd);
    app.primary
        .map
        .try_apply_edits(edits, &mut Timer::throwaway());

    let (_, disconnected_after) =
        connectivity::find_scc(&app.primary.map, PathConstraints::Pedestrian);
    app.primary
        .map
        .must_apply_edits(orig_edits, &mut Timer::throwaway());

    let newly_disconnected = disconnected_after
        .difference(&disconnected_before)
        .collect::<Vec<_>>();
    if newly_disconnected.is_empty() {
        return None;
    }

    // TODO Think through a proper UI for showing editing errors to the user and letting them
    // understand the problem. We used to just draw problems in red and mostly cover it up with the
    // popup.
    Some(PopupMsg::new_state(
        ctx,
        "Error",
        vec![format!(
            "Can't close this intersection; {} sidewalks disconnected",
            newly_disconnected.len()
        )],
    ))
}

#[allow(unused)]
// Could be caused by closing intersections, changing lane types, or reversing lanes
pub fn check_blackholes(
    ctx: &mut EventCtx,
    app: &mut App,
    cmd: EditCmd,
) -> Option<Box<dyn State<App>>> {
    let orig_edits = app.primary.map.get_edits().clone();
    let mut driving_ok_originally = BTreeSet::new();
    let mut biking_ok_originally = BTreeSet::new();
    for l in app.primary.map.all_lanes() {
        if !l.driving_blackhole {
            driving_ok_originally.insert(l.id);
        }
        if !l.biking_blackhole {
            biking_ok_originally.insert(l.id);
        }
    }

    let mut edits = orig_edits.clone();
    edits.commands.push(cmd);
    app.primary
        .map
        .try_apply_edits(edits, &mut Timer::throwaway());

    let mut newly_disconnected = BTreeSet::new();
    for l in connectivity::find_scc(&app.primary.map, PathConstraints::Car).1 {
        if driving_ok_originally.contains(&l) {
            newly_disconnected.insert(l);
        }
    }
    for l in connectivity::find_scc(&app.primary.map, PathConstraints::Bike).1 {
        if biking_ok_originally.contains(&l) {
            newly_disconnected.insert(l);
        }
    }
    app.primary
        .map
        .must_apply_edits(orig_edits, &mut Timer::throwaway());

    if newly_disconnected.is_empty() {
        return None;
    }

    Some(PopupMsg::new_state(
        ctx,
        "Error",
        vec![format!(
            "{} lanes have been disconnected",
            newly_disconnected.len()
        )],
    ))
}

/// Looks at all changed roads and makes sure sidewalk directions are correct -- this is easy for
/// the user to mix up. Returns a list of new fixes to apply on top of the original edits.
pub fn fix_sidewalk_direction(map: &Map) -> Vec<EditCmd> {
    let mut fixes = Vec::new();
    for cmd in &map.get_edits().commands {
        if let EditCmd::ChangeRoad { r, new, .. } = cmd {
            let mut fixed = new.clone();
            if fixed.lanes_ltr[0].lt.is_walkable() {
                fixed.lanes_ltr[0].dir = if map.get_config().driving_side == DrivingSide::Right {
                    Direction::Back
                } else {
                    Direction::Fwd
                };
            }
            if fixed.lanes_ltr.len() > 1 {
                let last = fixed.lanes_ltr.last_mut().unwrap();
                if last.lt.is_walkable() {
                    last.dir = if map.get_config().driving_side == DrivingSide::Right {
                        Direction::Fwd
                    } else {
                        Direction::Back
                    };
                }
            }
            if &fixed != new {
                fixes.push(EditCmd::ChangeRoad {
                    r: *r,
                    old: new.clone(),
                    new: fixed,
                });
            }
        }
    }
    fixes
}

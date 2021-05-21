use std::collections::BTreeSet;

use map_gui::tools::{ColorDiscrete, PopupMsg};
use map_model::{connectivity, EditCmd, PathConstraints};
use widgetry::{Color, EventCtx, State};

use crate::app::App;

// All of these take a candidate EditCmd to do, then see if it's valid. If they return None, it's
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
    app.primary.map.try_apply_edits(edits);

    let (_, disconnected_after) =
        connectivity::find_scc(&app.primary.map, PathConstraints::Pedestrian);
    app.primary.map.must_apply_edits(orig_edits);

    let newly_disconnected = disconnected_after
        .difference(&disconnected_before)
        .collect::<Vec<_>>();
    if newly_disconnected.is_empty() {
        return None;
    }

    let mut c = ColorDiscrete::new(app, vec![("disconnected", Color::RED)]);
    let num = newly_disconnected.len();
    for l in newly_disconnected {
        c.add_l(*l, "disconnected");
    }
    let (unzoomed, zoomed, _) = c.build(ctx);

    Some(PopupMsg::also_draw(
        ctx,
        "Error",
        vec![format!(
            "Can't close this intersection; {} sidewalks disconnected",
            num
        )],
        unzoomed,
        zoomed,
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
    for l in app.primary.map.all_lanes().values() {
        if !l.driving_blackhole {
            driving_ok_originally.insert(l.id);
        }
        if !l.biking_blackhole {
            biking_ok_originally.insert(l.id);
        }
    }

    let mut edits = orig_edits.clone();
    edits.commands.push(cmd);
    app.primary.map.try_apply_edits(edits);

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
    app.primary.map.must_apply_edits(orig_edits);

    if newly_disconnected.is_empty() {
        return None;
    }

    let mut c = ColorDiscrete::new(app, vec![("disconnected", Color::RED)]);
    let num = newly_disconnected.len();
    for l in newly_disconnected {
        c.add_l(l, "disconnected");
    }
    let (unzoomed, zoomed, _) = c.build(ctx);

    Some(PopupMsg::also_draw(
        ctx,
        "Error",
        vec![format!("{} lanes have been disconnected", num)],
        unzoomed,
        zoomed,
    ))
}

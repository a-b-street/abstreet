use crate::app::App;
use crate::common::ColorDiscrete;
use crate::game::{msg, State, WizardState};
use abstutil::Timer;
use ezgui::{Color, EventCtx};
use map_model::{connectivity, EditCmd, PathConstraints};
use std::collections::BTreeSet;

// All of these take a candidate EditCmd to do, then see if it's valid. If they return None, it's
// fine. They always leave the map in the original state without the new EditCmd.

// Could be caused by closing intersections
pub fn check_sidewalk_connectivity(
    ctx: &mut EventCtx,
    app: &mut App,
    cmd: EditCmd,
) -> Option<Box<dyn State>> {
    let orig_edits = app.primary.map.get_edits().clone();
    let (_, disconnected_before) =
        connectivity::find_scc(&app.primary.map, PathConstraints::Pedestrian);

    let mut edits = orig_edits.clone();
    edits.commands.push(cmd);
    app.primary.map.apply_edits(edits, &mut Timer::throwaway());

    let (_, disconnected_after) =
        connectivity::find_scc(&app.primary.map, PathConstraints::Pedestrian);
    app.primary
        .map
        .apply_edits(orig_edits, &mut Timer::throwaway());

    let newly_disconnected = disconnected_after
        .difference(&disconnected_before)
        .collect::<Vec<_>>();
    if newly_disconnected.is_empty() {
        return None;
    }

    let mut err_state = msg(
        "Error",
        vec![format!(
            "Can't close this intersection; {} sidewalks disconnected",
            newly_disconnected.len()
        )],
    );

    let mut c = ColorDiscrete::new(app, vec![("disconnected", Color::RED)]);
    for l in newly_disconnected {
        c.add_l(*l, "disconnected");
    }

    let (unzoomed, zoomed, _) = c.build(ctx);
    err_state.downcast_mut::<WizardState>().unwrap().also_draw = Some((unzoomed, zoomed));
    Some(err_state)
}

#[allow(unused)]
// Could be caused by closing intersections, changing lane types, or reversing lanes
pub fn check_parking_blackholes(
    ctx: &mut EventCtx,
    app: &mut App,
    cmd: EditCmd,
) -> Option<Box<dyn State>> {
    let orig_edits = app.primary.map.get_edits().clone();
    let mut ok_originally = BTreeSet::new();
    for l in app.primary.map.all_lanes() {
        if l.parking_blackhole.is_none() {
            ok_originally.insert(l.id);
            // TODO Only matters if there's any parking here anyways
        }
    }

    let mut edits = orig_edits.clone();
    edits.commands.push(cmd);
    app.primary.map.apply_edits(edits, &mut Timer::throwaway());

    let mut newly_disconnected = Vec::new();
    for (l, _) in
        connectivity::redirect_parking_blackholes(&app.primary.map, &mut Timer::throwaway())
    {
        if ok_originally.contains(&l) {
            newly_disconnected.push(l);
        }
    }
    app.primary
        .map
        .apply_edits(orig_edits, &mut Timer::throwaway());

    if newly_disconnected.is_empty() {
        return None;
    }

    let mut err_state = msg(
        "Error",
        vec![format!(
            "{} lanes have parking disconnected",
            newly_disconnected.len()
        )],
    );

    let mut c = ColorDiscrete::new(app, vec![("parking disconnected", Color::RED)]);
    for l in newly_disconnected {
        c.add_l(l, "parking disconnected");
    }

    let (unzoomed, zoomed, _) = c.build(ctx);
    err_state.downcast_mut::<WizardState>().unwrap().also_draw = Some((unzoomed, zoomed));
    Some(err_state)
}

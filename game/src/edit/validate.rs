use crate::app::App;
use crate::common::ColorDiscrete;
use crate::game::{msg, State, WizardState};
use abstutil::Timer;
use ezgui::{Color, EventCtx};
use map_model::{connectivity, EditCmd, LaneID, LaneType, Map, PathConstraints};
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
pub fn check_blackholes(ctx: &mut EventCtx, app: &mut App, cmd: EditCmd) -> Option<Box<dyn State>> {
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

    let mut err_state = msg(
        "Error",
        vec![format!(
            "{} lanes have been disconnected",
            newly_disconnected.len()
        )],
    );

    let mut c = ColorDiscrete::new(app, vec![("disconnected", Color::RED)]);
    for l in newly_disconnected {
        c.add_l(l, "disconnected");
    }

    let (unzoomed, zoomed, _) = c.build(ctx);
    err_state.downcast_mut::<WizardState>().unwrap().also_draw = Some((unzoomed, zoomed));
    Some(err_state)
}

pub fn try_change_lt(
    map: &mut Map,
    l: LaneID,
    new_lt: LaneType,
) -> Result<EditCmd, Box<dyn State>> {
    let orig_edits = map.get_edits().clone();

    let mut edits = orig_edits.clone();
    let cmd = EditCmd::ChangeLaneType {
        id: l,
        lt: new_lt,
        orig_lt: map.get_l(l).lane_type,
    };
    edits.commands.push(cmd.clone());
    map.try_apply_edits(edits, &mut Timer::throwaway());

    let mut errors = Vec::new();
    let r = map.get_parent(l);

    // Only one parking lane per side.
    if r.children(r.is_forwards(l))
        .iter()
        .filter(|(_, lt)| *lt == LaneType::Parking)
        .count()
        > 1
    {
        // TODO Actually, we just don't want two adjacent parking lanes
        // (What about dppd though?)
        errors.push(format!(
            "You can only have one parking lane on the same side of the road"
        ));
    }

    // A parking lane must have a driving lane somewhere on the road.
    let (fwd, back) = r.get_lane_types();
    let all_types: BTreeSet<LaneType> = fwd.chain(back).collect();
    if all_types.contains(&LaneType::Parking) && !all_types.contains(&LaneType::Driving) {
        errors.push(format!(
            "A parking lane needs a driving lane somewhere on the same road"
        ));
    }

    // Don't let players orphan a bus stop.
    if !r.all_bus_stops(map).is_empty()
        && !r
            .children(r.is_forwards(l))
            .iter()
            .any(|(_, lt)| *lt == LaneType::Driving || *lt == LaneType::Bus)
    {
        errors.push(format!("You need a driving or bus lane for the bus stop!"));
    }

    map.must_apply_edits(orig_edits, &mut Timer::throwaway());
    if errors.is_empty() {
        Ok(cmd)
    } else {
        Err(msg("Error", errors))
    }
}

pub fn try_reverse(map: &Map, l: LaneID) -> Result<EditCmd, Box<dyn State>> {
    let lane = map.get_l(l);
    if !lane.lane_type.is_for_moving_vehicles() {
        Err(msg(
            "Error",
            vec![format!("You can't reverse a {:?} lane", lane.lane_type)],
        ))
    } else if map.get_r(lane.parent).dir_and_offset(l).1 != 0 {
        Err(msg(
            "Error",
            vec!["You can only reverse the lanes next to the road's yellow center line"],
        ))
    } else {
        Ok(EditCmd::ReverseLane {
            l,
            dst_i: lane.src_i,
        })
    }
}

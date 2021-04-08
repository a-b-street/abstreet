use std::collections::BTreeSet;

use map_gui::tools::{ColorDiscrete, PopupMsg};
use map_model::{connectivity, EditCmd, LaneID, LaneType, Map, PathConstraints};
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

pub fn try_change_lt(
    ctx: &mut EventCtx,
    map: &mut Map,
    l: LaneID,
    new_lt: LaneType,
) -> Result<EditCmd, Box<dyn State<App>>> {
    let orig_edits = map.get_edits().clone();

    let mut edits = orig_edits.clone();
    let cmd = {
        let r = map.get_l(l).parent;
        map.edit_road_cmd(r, |new| {
            new.lanes_ltr[map.get_r(r).offset(l)].0 = new_lt;
        })
    };
    edits.commands.push(cmd.clone());
    map.try_apply_edits(edits);

    let mut errors = Vec::new();
    let r = map.get_parent(l);

    // TODO Ban two adjacent parking lanes (What about dppd though?)

    // A parking lane must have a driving lane somewhere on the road.
    let all_types: BTreeSet<LaneType> = r.lanes_ltr().into_iter().map(|(_, _, lt)| lt).collect();
    if all_types.contains(&LaneType::Parking) && !all_types.contains(&LaneType::Driving) {
        errors.push(format!(
            "A parking lane needs a driving lane somewhere on the same road"
        ));
    }

    // Don't let players orphan a bus stop.
    // TODO This allows a bus stop switching sides of the road. Really need to re-do bus matching
    // and make sure nothing's broken (https://github.com/a-b-street/abstreet/issues/93).
    if !r.all_bus_stops(map).is_empty()
        && !r
            .lanes_ltr()
            .into_iter()
            .any(|(l, _, _)| PathConstraints::Bus.can_use(map.get_l(l), map))
    {
        errors.push(format!("You need a driving or bus lane for the bus stop!"));
    }

    map.must_apply_edits(orig_edits);
    if errors.is_empty() {
        Ok(cmd)
    } else {
        Err(PopupMsg::new(ctx, "Error", errors))
    }
}

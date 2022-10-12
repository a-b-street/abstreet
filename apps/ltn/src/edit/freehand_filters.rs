use geom::PolyLine;
use widgetry::EventCtx;

use crate::edit::{EditMode, EditOutcome};
use crate::{
    after_edit, mut_edits, App, DiagonalFilter, FilterType, Neighbourhood, RoadFilter, Transition,
};

pub fn event(ctx: &mut EventCtx, app: &mut App, neighbourhood: &Neighbourhood) -> EditOutcome {
    if let EditMode::FreehandFilters(ref mut lasso) = app.session.edit_mode {
        if let Some(pl) = lasso.event(ctx) {
            // Reset the tool
            app.session.edit_mode = EditMode::Filters;
            EditOutcome::Transition(make_filters_along_path(ctx, app, neighbourhood, pl))
        } else {
            // Do this instead of EditOutcome::Nothing to interrupt other processing
            EditOutcome::Transition(Transition::Keep)
        }
    } else {
        unreachable!()
    }
}

fn make_filters_along_path(
    ctx: &mut EventCtx,
    app: &mut App,
    neighbourhood: &Neighbourhood,
    path: PolyLine,
) -> Transition {
    let mut oneways = Vec::new();
    let mut bus_roads = Vec::new();

    app.per_map.proposals.before_edit();
    for r in &neighbourhood.orig_perimeter.interior {
        if app.edits().roads.contains_key(r) {
            continue;
        }
        let road = app.per_map.map.get_r(*r);
        // Don't show error messages
        if road.is_deadend_for_driving(&app.per_map.map) {
            continue;
        }
        if let Some((pt, _)) = road.center_pts.intersection(&path) {
            if road.oneway_for_driving().is_some() {
                oneways.push(*r);
                continue;
            }

            let dist = road
                .center_pts
                .dist_along_of_point(pt)
                .map(|pair| pair.0)
                .unwrap_or(road.center_pts.length() / 2.0);

            if app.session.filter_type != FilterType::BusGate
                && !app.per_map.map.get_bus_routes_on_road(*r).is_empty()
            {
                bus_roads.push((*r, dist));
                continue;
            }

            mut_edits!(app)
                .roads
                .insert(*r, RoadFilter::new_by_user(dist, app.session.filter_type));
        }
    }
    for i in &neighbourhood.interior_intersections {
        if app.per_map.map.get_i(*i).polygon.intersects_polyline(&path) {
            // We probably won't guess the right one, but make an attempt
            DiagonalFilter::cycle_through_alternatives(app, *i);
        }
    }
    after_edit(ctx, app);

    if !oneways.is_empty() {
        Transition::Push(super::ResolveOneWayAndFilter::new_state(ctx, oneways))
    } else if !bus_roads.is_empty() {
        Transition::Push(super::ResolveBusGate::new_state(ctx, app, bus_roads))
    } else {
        Transition::Recreate
    }
}

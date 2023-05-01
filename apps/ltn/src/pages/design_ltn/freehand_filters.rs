use geom::PolyLine;
use map_model::{DiagonalFilter, FilterType, RoadFilter};
use widgetry::EventCtx;

use super::{modals, EditMode, EditOutcome};
use crate::{redraw_all_filters, App, Neighbourhood, Transition};

pub fn event(ctx: &mut EventCtx, app: &mut App, neighbourhood: &Neighbourhood) -> EditOutcome {
    if let EditMode::FreehandFilters(ref mut lasso) = app.session.edit_mode {
        if let Some(pl) = lasso.event(ctx) {
            // Reset the tool
            app.session.edit_mode = EditMode::Filters;
            make_filters_along_path(ctx, app, neighbourhood, pl)
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
) -> EditOutcome {
    let mut oneways = Vec::new();
    let mut bus_roads = Vec::new();

    let mut edits = app.per_map.map.get_edits().clone();
    for r in &neighbourhood.interior_roads {
        let road = app.per_map.map.get_r(*r);
        if road.modal_filter.is_some() {
            continue;
        }
        // Don't show error messages
        if road.is_deadend_for_driving(&app.per_map.map) {
            continue;
        }
        if let Some((pt, _)) = road.center_pts.intersection(&path) {
            let dist = road
                .center_pts
                .dist_along_of_point(pt)
                .map(|pair| pair.0)
                .unwrap_or(road.center_pts.length() / 2.0);

            if road.oneway_for_driving().is_some() {
                if app.session.layers.autofix_one_ways {
                    modals::fix_oneway_and_add_filter(ctx, app, &[(*r, dist)]);
                } else {
                    oneways.push((*r, dist));
                }
                continue;
            }

            let mut filter_type = app.session.filter_type;
            if filter_type != FilterType::BusGate
                && !app.per_map.map.get_bus_routes_on_road(*r).is_empty()
            {
                if app.session.layers.autofix_bus_gates {
                    filter_type = FilterType::BusGate;
                } else {
                    bus_roads.push((*r, dist));
                    continue;
                }
            }

            edits
                .commands
                .push(app.per_map.map.edit_road_cmd(*r, |new| {
                    new.modal_filter = Some(RoadFilter::new_by_user(dist, filter_type));
                }));
        }
    }
    for i in &neighbourhood.interior_intersections {
        if app.per_map.map.get_i(*i).polygon.intersects_polyline(&path) {
            // We probably won't guess the right one, but make an attempt
            edits
                .commands
                .extend(DiagonalFilter::cycle_through_alternatives(
                    &app.per_map.map,
                    *i,
                    app.session.filter_type,
                ));
        }
    }
    app.apply_edits(edits);
    redraw_all_filters(ctx, app);

    if !oneways.is_empty() {
        EditOutcome::Transition(Transition::Push(modals::ResolveOneWayAndFilter::new_state(
            ctx, oneways,
        )))
    } else if !bus_roads.is_empty() {
        EditOutcome::Transition(Transition::Push(modals::ResolveBusGate::new_state(
            ctx, app, bus_roads,
        )))
    } else {
        EditOutcome::UpdateAll
    }
}

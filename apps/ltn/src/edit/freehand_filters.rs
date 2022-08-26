use geom::PolyLine;
use widgetry::{EventCtx, Line, Text, Widget};

use crate::edit::{EditMode, EditOutcome};
use crate::{after_edit, App, DiagonalFilter, Neighbourhood, RoadFilter, Transition};

pub fn widget(ctx: &mut EventCtx) -> Widget {
    Text::from_all(vec![
        Line("Click and drag").fg(ctx.style().text_hotkey_color),
        Line(" across the roads you want to filter"),
    ])
    .wrap_to_pct(ctx, 15)
    .into_widget(ctx)
}

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

    app.session.edits.before_edit();
    for r in &neighbourhood.orig_perimeter.interior {
        if app.session.edits.roads.contains_key(r) {
            continue;
        }
        let road = app.map.get_r(*r);
        // Don't show error messages
        if road.is_deadend_for_driving(&app.map) {
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
            app.session
                .edits
                .roads
                .insert(*r, RoadFilter::new_by_user(dist, app.session.filter_type));
        }
    }
    for i in &neighbourhood.interior_intersections {
        if app.map.get_i(*i).polygon.intersects_polyline(&path) {
            // We probably won't guess the right one, but make an attempt
            DiagonalFilter::cycle_through_alternatives(app, *i);
        }
    }
    after_edit(ctx, app);

    if oneways.is_empty() {
        Transition::Recreate
    } else {
        Transition::Push(super::ResolveOneWayAndFilter::new_state(ctx, oneways))
    }
}

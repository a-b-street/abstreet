use widgetry::mapspace::{World, WorldOutcome};
use widgetry::tools::open_browser;
use widgetry::{lctrl, EventCtx, Key, Line, Text, Transition, Widget};

use super::{road_name, EditOutcome, Obj};
use crate::{after_edit, colors, App, DiagonalFilter, Neighbourhood, RoadFilter};

pub fn widget(ctx: &mut EventCtx) -> Widget {
    Text::from(Line(
        "Click a road or intersection to add or remove a modal filter",
    ))
    .wrap_to_pct(ctx, 15)
    .into_widget(ctx)
}

/// Creates clickable objects for managing filters on roads and intersections. Everything is
/// invisible; the caller is responsible for drawing things.
pub fn make_world(ctx: &mut EventCtx, app: &App, neighbourhood: &Neighbourhood) -> World<Obj> {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());

    for r in &neighbourhood.orig_perimeter.interior {
        let road = map.get_r(*r);
        world
            .add(Obj::InteriorRoad(*r))
            .hitbox(road.get_thick_polygon())
            .drawn_in_master_batch()
            .hover_color(colors::HOVER)
            .tooltip(Text::from(format!(
                "{} possible shortcuts cross {}",
                neighbourhood.shortcuts.count_per_road.get(*r),
                road_name(app, road)
            )))
            .hotkey(lctrl(Key::D), "debug")
            .clickable()
            .build(ctx);
    }

    for i in &neighbourhood.interior_intersections {
        world
            .add(Obj::InteriorIntersection(*i))
            .hitbox(map.get_i(*i).polygon.clone())
            .drawn_in_master_batch()
            .hover_color(colors::HOVER)
            .tooltip(Text::from(format!(
                "{} possible shortcuts cross this intersection",
                neighbourhood.shortcuts.count_per_intersection.get(*i)
            )))
            .clickable()
            .hotkey(lctrl(Key::D), "debug")
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}

pub fn handle_world_outcome(
    ctx: &mut EventCtx,
    app: &mut App,
    outcome: WorldOutcome<Obj>,
) -> EditOutcome {
    let map = &app.map;
    match outcome {
        WorldOutcome::ClickedObject(Obj::InteriorRoad(r)) => {
            let road = map.get_r(r);
            // The world doesn't contain non-driveable roads, so no need to check for that error
            if road.oneway_for_driving().is_some() {
                return EditOutcome::error(ctx, "A one-way street can't have a filter");
            }
            if road.is_deadend_for_driving(&app.map) {
                return EditOutcome::error(ctx, "You can't filter a dead-end");
            }

            app.session.edits.before_edit();
            if app.session.edits.roads.remove(&r).is_none() {
                // Place the filter on the part of the road that was clicked
                // These calls shouldn't fail -- since we clicked a road, the cursor must be in
                // map-space. And project_pt returns a point that's guaranteed to be on the
                // polyline.
                let cursor_pt = ctx.canvas.get_cursor_in_map_space().unwrap();
                let pt_on_line = road.center_pts.project_pt(cursor_pt);
                let (distance, _) = road.center_pts.dist_along_of_point(pt_on_line).unwrap();

                app.session.edits.roads.insert(
                    r,
                    RoadFilter::new_by_user(distance, app.session.filter_type),
                );
            }
            after_edit(ctx, app);
            EditOutcome::Transition(Transition::Recreate)
        }
        WorldOutcome::ClickedObject(Obj::InteriorIntersection(i)) => {
            app.session.edits.before_edit();
            DiagonalFilter::cycle_through_alternatives(app, i);
            after_edit(ctx, app);
            EditOutcome::Transition(Transition::Recreate)
        }
        WorldOutcome::Keypress("debug", Obj::InteriorIntersection(i)) => {
            open_browser(app.map.get_i(i).orig_id.to_string());
            EditOutcome::Nothing
        }
        WorldOutcome::Keypress("debug", Obj::InteriorRoad(r)) => {
            open_browser(app.map.get_r(r).orig_id.osm_way_id.to_string());
            EditOutcome::Nothing
        }
        _ => EditOutcome::Nothing,
    }
}

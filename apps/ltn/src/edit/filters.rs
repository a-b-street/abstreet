use geom::Distance;
use map_model::PathConstraints;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::tools::open_browser;
use widgetry::{lctrl, EventCtx, Image, Key, Line, Text, TextExt, Widget};

use super::Obj;
use crate::shortcuts::Shortcuts;
use crate::{after_edit, colors, App, DiagonalFilter, Neighbourhood};

pub fn widget(ctx: &mut EventCtx, app: &App) -> Widget {
    Widget::col(vec![
        Widget::row(vec![
            Image::from_path("system/assets/tools/pencil.svg")
                .into_widget(ctx)
                .centered_vert(),
            Text::from(Line(
                "Click a road or intersection to add or remove a modal filter",
            ))
            .wrap_to_pct(ctx, 15)
            .into_widget(ctx),
        ]),
        crate::components::FreehandFilters::button(ctx),
        Widget::row(vec![
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/undo.svg")
                .disabled(app.session.modal_filters.previous_version.is_none())
                .hotkey(lctrl(Key::Z))
                .build_widget(ctx, "undo"),
            format!(
                "{} filters added",
                app.session.modal_filters.roads.len()
                    + app.session.modal_filters.intersections.len()
            )
            .text_widget(ctx)
            .centered_vert(),
        ]),
    ])
}

/// Creates clickable objects for managing filters on roads and intersections. Everything is
/// invisible; the caller is responsible for drawing things.
pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
    shortcuts: &Shortcuts,
) -> World<Obj> {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());

    for r in &neighbourhood.orig_perimeter.interior {
        let road = map.get_r(*r);
        world
            .add(Obj::InteriorRoad(*r))
            .hitbox(road.get_thick_polygon())
            .drawn_in_master_batch()
            .hover_outline(colors::OUTLINE, Distance::meters(5.0))
            .tooltip(Text::from(format!(
                "{} shortcuts cross {}",
                shortcuts.count_per_road.get(*r),
                road.get_name(app.opts.language.as_ref()),
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
            .hover_outline(colors::OUTLINE, Distance::meters(5.0))
            .tooltip(Text::from(format!(
                "{} shortcuts cross this intersection",
                shortcuts.count_per_intersection.get(*i)
            )))
            .clickable()
            .hotkey(lctrl(Key::D), "debug")
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}

pub fn handle_world_outcome(ctx: &mut EventCtx, app: &mut App, outcome: WorldOutcome<Obj>) -> bool {
    let map = &app.map;
    match outcome {
        WorldOutcome::ClickedObject(Obj::InteriorRoad(r)) => {
            let road = map.get_r(r);
            // Filtering a road that's already marked bike-only doesn't make sense. Likewise for
            // one-ways.
            if !PathConstraints::Car.can_use_road(road, map) || road.oneway_for_driving().is_some()
            {
                return true;
            }

            app.session.modal_filters.before_edit();
            if app.session.modal_filters.roads.remove(&r).is_none() {
                // Place the filter on the part of the road that was clicked
                // These calls shouldn't fail -- since we clicked a road, the cursor must be in
                // map-space. And project_pt returns a point that's guaranteed to be on the
                // polyline.
                let cursor_pt = ctx.canvas.get_cursor_in_map_space().unwrap();
                let pt_on_line = road.center_pts.project_pt(cursor_pt);
                let (distance, _) = road.center_pts.dist_along_of_point(pt_on_line).unwrap();

                app.session.modal_filters.roads.insert(r, distance);
            }
            after_edit(ctx, app);
            true
        }
        WorldOutcome::ClickedObject(Obj::InteriorIntersection(i)) => {
            app.session.modal_filters.before_edit();
            DiagonalFilter::cycle_through_alternatives(app, i);
            after_edit(ctx, app);
            true
        }
        WorldOutcome::Keypress("debug", Obj::InteriorIntersection(i)) => {
            open_browser(app.map.get_i(i).orig_id.to_string());
            false
        }
        WorldOutcome::Keypress("debug", Obj::InteriorRoad(r)) => {
            open_browser(app.map.get_r(r).orig_id.osm_way_id.to_string());
            false
        }
        _ => false,
    }
}

use geom::Distance;
use raw_map::LaneSpec;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, Image, Text, TextExt, Transition, Widget};

use super::{EditOutcome, Obj};
use crate::{colors, App, Neighbourhood};

pub fn widget(ctx: &mut EventCtx) -> Widget {
    Widget::col(vec![
        Widget::row(vec![
            Image::from_path("system/assets/tools/pencil.svg")
                .into_widget(ctx)
                .centered_vert(),
            "Click a road to change its direction".text_widget(ctx),
        ]),
        // TODO edit/undo?
    ])
}

pub fn make_world(ctx: &mut EventCtx, app: &App, neighbourhood: &Neighbourhood) -> World<Obj> {
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
                "Click to flip direction of {}",
                road.get_name(app.opts.language.as_ref()),
            )))
            .clickable()
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
    match outcome {
        WorldOutcome::ClickedObject(Obj::InteriorRoad(r)) => {
            if app.session.modal_filters.roads.contains_key(&r) {
                return EditOutcome::error(ctx, "A one-way street can't have a filter");
            }
            if app.map.get_r(r).is_deadend_for_driving(&app.map) {
                return EditOutcome::error(ctx, "A dead-end street can't be one-way");
            }

            let driving_side = app.map.get_config().driving_side;
            let mut edits = app.map.get_edits().clone();
            edits.commands.push(app.map.edit_road_cmd(r, |new| {
                LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
            }));

            ctx.loading_screen("apply edits", |_, timer| {
                let effects = app.map.must_apply_edits(edits, timer);
                // We don't need to regenerate_unzoomed_layer for one-ways; no widths or styling
                // has changed
                for r in effects.changed_roads {
                    let road = app.map.get_r(r);
                    app.draw_map.recreate_road(road, &app.map);
                }
                for i in effects.changed_intersections {
                    app.draw_map.recreate_intersection(i, &app.map);
                }

                // See the argument in filters/existing.rs about not recalculating the pathfinder.
                app.map.keep_pathfinder_despite_edits();
            });

            EditOutcome::Transition(Transition::Recreate)
        }
        _ => EditOutcome::Nothing,
    }
}

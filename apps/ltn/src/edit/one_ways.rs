use map_model::EditRoad;
use street_network::LaneSpec;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, Text, TextExt, Transition, Widget};

use super::{EditOutcome, Obj};
use crate::{colors, App, Neighbourhood};

pub fn widget(ctx: &mut EventCtx) -> Widget {
    "Click a road to change its direction".text_widget(ctx)
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
            .hover_color(colors::HOVER)
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
            if app.session.edits.roads.contains_key(&r) {
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
                app.map.must_apply_edits(edits, timer);
                // We don't need to regenerate_unzoomed_layer for one-ways; no widths or styling
                // has changed

                // See the argument in filters/existing.rs about not recalculating the pathfinder.
                // We always create it from-scratch when needed.
            });

            app.session.edits.before_edit();

            let r_edit = app.map.get_r_edit(r);
            // Was the road originally like this? Use the original OSM tags to decide.
            // TODO This'll break in the face of newer osm2streets transformations. But it's the
            // same problem as EditRoad::get_orig_from_osm -- figure out a bigger solution later.
            if r_edit == EditRoad::get_orig_from_osm(app.map.get_r(r), app.map.get_config()) {
                app.session.edits.one_ways.remove(&r);
            } else {
                app.session.edits.one_ways.insert(r, r_edit);
            }

            // We don't need to call after_edit; no filter icons have changed

            EditOutcome::Transition(Transition::Recreate)
        }
        _ => EditOutcome::Nothing,
    }
}

// This is defined here because some of the heavy lifting deals with one-ways, but it might not
// even undo that kind of change
pub fn undo_proposal(ctx: &mut EventCtx, app: &mut App) {
    let prev = app.session.edits.previous_version.take().unwrap();

    // Generate edits to undo the one possible change to a one-way
    if prev.one_ways != app.session.edits.one_ways {
        let mut edits = app.map.get_edits().clone();
        // We can actually just cheat a bit -- it must be the last command
        edits.commands.pop().unwrap();
        ctx.loading_screen("apply edits", |_, timer| {
            app.map.must_apply_edits(edits, timer);
        });
    }

    app.session.edits = prev;
    crate::after_edit(ctx, app);
}

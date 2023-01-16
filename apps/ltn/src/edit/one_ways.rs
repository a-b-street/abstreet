use map_model::EditRoad;
use osm2streets::LaneSpec;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, Text, Transition};

use super::{road_name, EditOutcome, Obj};
use crate::{colors, mut_edits, App, Neighbourhood};

pub fn make_world(ctx: &mut EventCtx, app: &App, neighbourhood: &Neighbourhood) -> World<Obj> {
    let map = &app.per_map.map;
    let mut world = World::bounded(map.get_bounds());

    for r in &neighbourhood.interior_roads {
        let road = map.get_r(*r);
        world
            .add(Obj::InteriorRoad(*r))
            .hitbox(road.get_thick_polygon())
            .drawn_in_master_batch()
            .hover_color(colors::HOVER)
            .tooltip(Text::from(format!(
                "Click to flip direction of {}",
                road_name(app, road)
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
            if app.edits().roads.contains_key(&r) {
                return EditOutcome::error(ctx, "A one-way street can't have a filter");
            }
            if app
                .per_map
                .map
                .get_r(r)
                .is_deadend_for_driving(&app.per_map.map)
            {
                return EditOutcome::error(ctx, "A dead-end street can't be one-way");
            }

            let driving_side = app.per_map.map.get_config().driving_side;
            let mut edits = app.per_map.map.get_edits().clone();
            edits.commands.push(app.per_map.map.edit_road_cmd(r, |new| {
                LaneSpec::toggle_road_direction(&mut new.lanes_ltr, driving_side);
            }));

            ctx.loading_screen("apply edits", |_, timer| {
                app.per_map.map.must_apply_edits(edits, timer);
                // We don't need to regenerate_unzoomed_layer for one-ways; no widths or styling
                // has changed

                // See the argument in filters/existing.rs about not recalculating the pathfinder.
                // We always create it from-scratch when needed.
            });

            app.per_map.proposals.before_edit();

            let r_edit = app.per_map.map.get_r_edit(r);
            // Was the road originally like this? Use the original OSM tags to decide.
            // TODO This'll break in the face of newer osm2streets transformations. But it's the
            // same problem as EditRoad::get_orig_from_osm -- figure out a bigger solution later.
            if r_edit
                == EditRoad::get_orig_from_osm(
                    app.per_map.map.get_r(r),
                    app.per_map.map.get_config(),
                )
            {
                mut_edits!(app).one_ways.remove(&r);
            } else {
                mut_edits!(app).one_ways.insert(r, r_edit);
            }

            // We don't need to call redraw_all_filters; no icons have changed

            EditOutcome::Transition(Transition::Recreate)
        }
        _ => EditOutcome::Nothing,
    }
}

// This is defined here because some of the heavy lifting deals with one-ways, but it might not
// even undo that kind of change
pub fn undo_proposal(ctx: &mut EventCtx, app: &mut App) {
    // use before_edit to maybe fork the proposal, but then we need to undo the no-op change it
    // pushes onto edit history
    app.per_map.proposals.before_edit();
    mut_edits!(app) = mut_edits!(app).previous_version.take().unwrap();

    // This is the real previous state that we'll rollback to
    let prev = mut_edits!(app).previous_version.take().unwrap();

    // Generate edits to undo possible changes to a one-way. Note there may be multiple in one
    // batch, from the freehand tool
    if prev.one_ways != app.edits().one_ways {
        let mut edits = app.per_map.map.get_edits().clone();

        for (r, r_edit1) in &prev.one_ways {
            if Some(r_edit1) != app.edits().one_ways.get(r) {
                edits
                    .commands
                    .push(app.per_map.map.edit_road_cmd(*r, |new| {
                        *new = r_edit1.clone();
                    }));
            }
        }
        // Also look for newly introduced one-ways
        for r in app.edits().one_ways.keys() {
            if !prev.one_ways.contains_key(r) {
                edits
                    .commands
                    .push(app.per_map.map.edit_road_cmd(*r, |new| {
                        *new = EditRoad::get_orig_from_osm(
                            app.per_map.map.get_r(*r),
                            app.per_map.map.get_config(),
                        );
                    }));
            }
        }

        ctx.loading_screen("apply edits", |_, timer| {
            app.per_map.map.must_apply_edits(edits, timer);
        });
    }

    mut_edits!(app) = prev;
    crate::redraw_all_filters(ctx, app);
}

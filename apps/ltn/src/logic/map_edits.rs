use map_model::{EditRoad, MapEdits, RoadID};
use widgetry::EventCtx;

use crate::{mut_edits, App};

pub fn modify_road(ctx: &mut EventCtx, app: &mut App, r: RoadID, edits: MapEdits) {
    ctx.loading_screen("apply edits", |_, timer| {
        app.per_map.map.must_apply_edits(edits, timer);
        // We don't need to regenerate_unzoomed_layer for one-ways or speed limits; no widths or
        // styling has changed

        // See the argument in filters/existing.rs about not recalculating the pathfinder.
        // We always create it from-scratch when needed.
    });

    app.per_map.proposals.before_edit();

    let r_edit = app.per_map.map.get_r_edit(r);
    // Was the road originally like this? Use the original OSM tags to decide.
    // TODO This'll break in the face of newer osm2streets transformations. But it's the
    // same problem as EditRoad::get_orig_from_osm -- figure out a bigger solution later.
    if r_edit == EditRoad::get_orig_from_osm(app.per_map.map.get_r(r), app.per_map.map.get_config())
    {
        mut_edits!(app).one_ways.remove(&r);
    } else {
        mut_edits!(app).one_ways.insert(r, r_edit);
    }

    // We don't need to call redraw_all_filters; no icons have changed
}

pub fn undo_proposal(ctx: &mut EventCtx, app: &mut App) {
    // use before_edit to maybe fork the proposal, but then we need to undo the no-op change it
    // pushes onto edit history
    app.per_map.proposals.before_edit();
    mut_edits!(app) = mut_edits!(app).previous_version.take().unwrap();

    // This is the real previous state that we'll rollback to
    let prev = mut_edits!(app).previous_version.take().unwrap();

    // Generate edits to undo possible changes to a one-way or speed limit. Note there may be
    // multiple in one batch, from the freehand tool
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
        // Also look for newly introduced one-ways or speed limit changes
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

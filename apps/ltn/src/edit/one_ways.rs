use geom::Distance;
use raw_map::{Direction, DrivingSide, LaneSpec, LaneType};
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, Image, Text, TextExt, Widget};

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

            let leftmost_dir = if app.map.get_config().driving_side == DrivingSide::Right {
                Direction::Back
            } else {
                Direction::Fwd
            };

            let mut edits = app.map.get_edits().clone();
            edits.commands.push(app.map.edit_road_cmd(r, |new| {
                // TODO Refactor transformations like this into raw_map. Ungap's maybe_add_bike_lanes is another!

                let oneway_dir = LaneSpec::oneway_for_driving(&new.lanes_ltr);
                let mut num_driving_lanes = new
                    .lanes_ltr
                    .iter()
                    .filter(|lane| lane.lt == LaneType::Driving)
                    .count();

                // Pre-processing: if it's one-way backwards and there's only one driving lane,
                // split it into two narrow pieces
                if oneway_dir == Some(Direction::Back) && num_driving_lanes == 1 {
                    // TODO If there's parking, use that
                    let idx = new
                        .lanes_ltr
                        .iter()
                        .position(|x| x.lt == LaneType::Driving)
                        .unwrap();
                    new.lanes_ltr[idx].width *= 0.5;
                    new.lanes_ltr.insert(idx, new.lanes_ltr[idx].clone());
                    num_driving_lanes = 2;
                }
                // And undo the above
                if oneway_dir == None && num_driving_lanes == 2 {
                    let idx = new
                        .lanes_ltr
                        .iter()
                        .position(|x| x.lt == LaneType::Driving)
                        .unwrap();
                    // Is it super narrow?
                    // TODO Potentially brittle. SERVICE_ROAD_LANE_THICKNESS is 1.5,
                    // NORMAL_LANE_THICKNESS is 2.5. Half of either one is less than 1.5.
                    if new.lanes_ltr[idx].width < Distance::meters(1.5) {
                        new.lanes_ltr.remove(idx);
                        new.lanes_ltr[idx].width *= 2.0;
                    }
                }

                let mut driving_lanes_so_far = 0;
                for lane in &mut new.lanes_ltr {
                    if lane.lt == LaneType::Driving {
                        driving_lanes_so_far += 1;
                        match oneway_dir {
                            Some(Direction::Fwd) => {
                                // If it's one-way forwards, flip the direction
                                lane.dir = Direction::Back;
                            }
                            Some(Direction::Back) => {
                                // If it's one-way backwards, make it bidirectional. Split the
                                // directions down the middle
                                if (driving_lanes_so_far as f64) / (num_driving_lanes as f64) <= 0.5
                                {
                                    lane.dir = leftmost_dir;
                                } else {
                                    lane.dir = leftmost_dir.opposite();
                                }
                            }
                            None => {
                                // TODO If it's narrow...
                                // If it's bidirectional, make it one-way
                                lane.dir = Direction::Fwd;
                            }
                        }
                    }
                }
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

            EditOutcome::Recalculate
        }
        _ => EditOutcome::Nothing,
    }
}

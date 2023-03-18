use osm2streets::LaneSpec;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, Text};

use super::{road_name, EditOutcome, Obj};
use crate::render::colors;
use crate::{logic, App, Neighbourhood};

pub fn make_world(ctx: &mut EventCtx, app: &App, neighbourhood: &Neighbourhood) -> World<Obj> {
    let map = &app.per_map.map;
    let mut world = World::new();

    for r in &neighbourhood.interior_roads {
        let road = map.get_r(*r);
        world
            .add(Obj::Road(*r))
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
        WorldOutcome::ClickedObject(Obj::Road(r)) => {
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

            logic::map_edits::modify_road(ctx, app, r, edits);

            EditOutcome::UpdateAll
        }
        _ => EditOutcome::Nothing,
    }
}

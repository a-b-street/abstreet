use geom::{Speed, UnitFmt};
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::tools::ColorLegend;
use widgetry::{EventCtx, Text, Transition, Widget};

use super::{EditOutcome, Obj};
use crate::{colors, App, Neighbourhood};

pub fn widget(ctx: &mut EventCtx) -> Widget {
    ColorLegend::categories(
        ctx,
        vec![
            (colors::SPEED_LIMITS[0], "<= 10mph"),
            (colors::SPEED_LIMITS[1], "<= 20mph"),
            (colors::SPEED_LIMITS[2], "<= 30mph"),
            (colors::SPEED_LIMITS[3], "> 30mph"),
        ],
    )
}

pub fn make_world(ctx: &mut EventCtx, app: &App, neighbourhood: &Neighbourhood) -> World<Obj> {
    let map = &app.per_map.map;
    let mut world = World::new();

    for r in neighbourhood
        .interior_roads
        .iter()
        .chain(neighbourhood.perimeter_roads.iter())
    {
        let road = map.get_r(*r);
        let s = road.speed_limit.to_miles_per_hour().round();

        world
            .add(Obj::Road(*r))
            .hitbox(road.get_thick_polygon())
            .draw_color(if s <= 10.0 {
                colors::SPEED_LIMITS[0]
            } else if s <= 20.0 {
                colors::SPEED_LIMITS[1]
            } else if s <= 30.0 {
                colors::SPEED_LIMITS[2]
            } else {
                colors::SPEED_LIMITS[3]
            })
            .hover_color(colors::HOVER)
            .tooltip(Text::from(format!(
                "Current speed limit is {} ({})",
                road.speed_limit.to_string(&UnitFmt::imperial()),
                road.speed_limit.to_string(&UnitFmt::metric()),
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
            if app.per_map.map.get_r(r).speed_limit == Speed::miles_per_hour(20.0) {
                return EditOutcome::Nothing;
            }

            let mut edits = app.per_map.map.get_edits().clone();
            edits.commands.push(app.per_map.map.edit_road_cmd(r, |new| {
                new.speed_limit = Speed::miles_per_hour(20.0);
            }));

            super::map_edits::modify_road(ctx, app, r, edits);

            EditOutcome::Transition(Transition::Recreate)
        }
        _ => EditOutcome::Nothing,
    }
}

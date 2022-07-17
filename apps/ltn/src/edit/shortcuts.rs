use geom::Distance;
use map_model::RoadID;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{Color, EventCtx, Text, TextExt, Widget};

use super::{EditMode, EditOutcome, Obj};
use crate::shortcuts::Shortcuts;
use crate::{colors, App, Neighbourhood};

pub fn widget(
    ctx: &mut EventCtx,
    app: &App,
    shortcuts: &Shortcuts,
    focus: Option<RoadID>,
) -> Widget {
    match focus {
        Some(r) => Widget::col(vec![format!(
            "{} possible shortcuts cross {}",
            shortcuts.count_per_road.get(r),
            app.map.get_r(r).get_name(app.opts.language.as_ref()),
        )
        .text_widget(ctx)]),
        None => Widget::col(vec![
            "Click a road to view shortcuts through it".text_widget(ctx)
        ]),
    }
}

pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
    shortcuts: &Shortcuts,
    focus: Option<RoadID>,
) -> World<Obj> {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());

    for r in &neighbourhood.orig_perimeter.interior {
        let road = map.get_r(*r);
        if focus == Some(*r) {
            world
                .add(Obj::InteriorRoad(*r))
                .hitbox(road.get_thick_polygon())
                .draw_color(Color::BLUE)
                .build(ctx);
        } else {
            world
                .add(Obj::InteriorRoad(*r))
                .hitbox(road.get_thick_polygon())
                .drawn_in_master_batch()
                .hover_outline(colors::OUTLINE, Distance::meters(5.0))
                .tooltip(Text::from(format!(
                    "{} possible shortcuts cross {}",
                    shortcuts.count_per_road.get(*r),
                    road.get_name(app.opts.language.as_ref()),
                )))
                .clickable()
                .build(ctx);
        }
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
            app.session.edit_mode = EditMode::Shortcuts(Some(r));
            // TODO make the scroller thing

            EditOutcome::Recalculate
        }
        WorldOutcome::ClickedFreeSpace(_) => {
            app.session.edit_mode = EditMode::Shortcuts(None);
            EditOutcome::Recalculate
        }
        _ => EditOutcome::Nothing,
    }
}

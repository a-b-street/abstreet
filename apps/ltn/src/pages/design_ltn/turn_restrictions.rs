use geom::Distance;
use map_model::{PathV2, RoadID};
use osm2streets::RestrictionType;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{Color, EventCtx, GeomBatch, Key, Line, Text, TextExt, Widget};

use super::{road_name, EditMode, EditOutcome, Obj};
use crate::render::colors;
use crate::{App, Neighbourhood};

use super::shortcuts::FocusedRoad;


pub fn widget(ctx: &mut EventCtx, app: &App, focus: Option<&FocusedRoad>) -> Widget {
    match focus {
        Some(focus) => Widget::col(vec![
            format!(
                "Turn Restrictions from {}",
                app.per_map
                    .map
                    .get_r(focus.r)
                    .get_name(app.opts.language.as_ref()),
            )
            .text_widget(ctx),
        ]),
        None => Widget::nothing(),
    }
}

pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
    focus: &Option<FocusedRoad>,
) -> World<Obj> {
    let map = &app.per_map.map;
    let mut world = World::new();
    let focused_road = focus.as_ref().map(|f| f.r);

    let mut restricted_destinations: Vec<RoadID> = Vec::new();
    if focused_road.is_some() {
        let focused_r = map.get_r(focused_road.unwrap());
        for (restriction, r2) in &focused_r.turn_restrictions {
            if *restriction == RestrictionType::BanTurns {
                restricted_destinations.push(*r2);
            }
        }
        for (via, r2) in &focused_r.complicated_turn_restrictions {
            // TODO Show the 'via'? Or just draw the entire shape?
            restricted_destinations.push(*via);
            restricted_destinations.push(*r2);
        }
    }

    println!("TURN RESTRICTIONS: make_world :{:?}", focused_road);
    println!("TURN RESTRICTIONS: restricted_destinations :{:?}", restricted_destinations);
    for r in &neighbourhood.interior_roads {
        let road = map.get_r(*r);
        if focused_road == Some(*r) {
            let mut batch = GeomBatch::new();
            batch.push(
                Color::RED,
                road.get_thick_polygon().to_outline(Distance::meters(3.0)),
            );

            world
                .add(Obj::Road(*r))
                .hitbox(road.get_thick_polygon())
                .draw(batch)
                .build(ctx);
        } else if restricted_destinations.contains(r) {
            let mut batch = GeomBatch::new();
            batch.push(
                Color::BLUE,
                road.get_thick_polygon().to_outline(Distance::meters(3.0)),
            );

            world
                .add(Obj::Road(*r))
                .hitbox(road.get_thick_polygon())
                .draw(batch)
                .build(ctx);

        } else {
            world
                .add(Obj::Road(*r))
                .hitbox(road.get_thick_polygon())
                .draw_color(colors::LOCAL_ROAD_LABEL.invert())
                .hover_color(colors::HOVER)
                .tooltip(Text::from(format!(
                    "{}",
                    road_name(app, road)
                )))
                .clickable()
                .build(ctx);
        }
    }

    // TODO
    // Highlight the current prohibited destination roads
    // Highlight the potential prohibited destination roads

    // world.initialize_hover(ctx);
    world
}

pub fn handle_world_outcome(
    app: &mut App,
    outcome: WorldOutcome<Obj>,
    neighbourhood: &Neighbourhood,
) -> EditOutcome {
        println!("TURN RESTRICTIONS: handle_world_outcome");

    match outcome {
        WorldOutcome::ClickedObject(Obj::Road(r)) => {
            // TODO - add logic based on which raod is clicked
            // Check if the ClickedObject is already highlighted
            // If so, then we should unhighlight it
            // If not and is one of the current prohibited destination roads, 
            //      then we should remove that prohibited turn
            // If not and is one of the potential prohibited destination roads,
            //      then we should add that prohibited turn


            let subset = neighbourhood.shortcuts.subset(neighbourhood, r);
            if subset.paths.is_empty() {
                EditOutcome::Nothing
            } else {
                app.session.edit_mode = EditMode::TurnRestrictions(Some(FocusedRoad {
                    r,
                    paths: subset.paths,
                    current_idx: 0,
                }));
                EditOutcome::UpdatePanelAndWorld
            }
        }
        WorldOutcome::ClickedFreeSpace(_) => {
            app.session.edit_mode = EditMode::TurnRestrictions(None);
            EditOutcome::UpdatePanelAndWorld
        }
        _ => EditOutcome::Nothing,
    }
}

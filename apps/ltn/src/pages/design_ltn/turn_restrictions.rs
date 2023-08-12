use geom::Distance;
use map_model::RoadID;
use osm2streets::RestrictionType;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, GeomBatch, Text, TextExt, Widget};

use super::{road_name, EditMode, EditOutcome, Obj};
use crate::logic::turn_restrictions::destination_roads;
use crate::render::colors;
use crate::{App, Neighbourhood};
use map_model::IntersectionID;

pub struct FocusedTurns {
    pub r: RoadID,
    pub i: IntersectionID,
}

pub fn widget(ctx: &mut EventCtx, app: &App, focus: Option<&FocusedTurns>) -> Widget {
    match focus {
        Some(focus) => Widget::col(vec![format!(
            "Turn Restrictions from {}",
            app.per_map
                .map
                .get_r(focus.r)
                .get_name(app.opts.language.as_ref()),
        )
        .text_widget(ctx)]),
        None => Widget::nothing(),
    }
}

pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
    focus: &Option<FocusedTurns>,
) -> World<Obj> {
    let map = &app.per_map.map;
    let mut world = World::new();
    let focused_road = focus.as_ref().map(|f| f.r);

    let all_r_id = [
        &neighbourhood.perimeter_roads,
        &neighbourhood.interior_roads,
        &neighbourhood.connected_exterior_roads,
    ]
    .into_iter()
    .flatten();

    for r in all_r_id {
        // for r in &neighbourhood.interior_roads {
        let road = map.get_r(*r);

        let mut restricted_destinations: Vec<&RoadID> = Vec::new();
        
        for (restriction, r2) in &road.turn_restrictions {
            if *restriction == RestrictionType::BanTurns {
                restricted_destinations.push(r2);
            }
        }
        for (via, r2) in &road.complicated_turn_restrictions {
            // TODO Show the 'via'? Or just draw the entire shape?
            restricted_destinations.push(via);
            restricted_destinations.push(r2);
        }

        // Account for one way streets when determining possible destinations
        // TODO This accounts for the oneway direction of the source street,
        // but not the oneway direction of the destination street
        let possible_destinations = destination_roads(map, road.id);


        let mut hover_batch = GeomBatch::new();
        // Create a single compound geometry which represents a Road *and its connected roads* and draw
        // that geom as the mouseover geom for the Road. This avoids needing to update the representation of 
        // any Roads other then FocusedRoad.
        // Add focus road segment itself
        hover_batch.push(
            colors::HOVER,
            road.get_thick_polygon(),
        );

        // Add possible destinations
        for possible_r in possible_destinations.clone() {
            let possible_road = map.get_r(possible_r);
            hover_batch.push(
                colors::TURN_PERMITTED_DESTINATION,
                possible_road.get_thick_polygon()
            );
        }

        // Add restricted_destinations
        for restricted_r in restricted_destinations.clone() {
            let restricted_road = map.get_r(*restricted_r);
            hover_batch.push(
                colors::TURN_PROHIBITED_DESTINATION,
                restricted_road.get_thick_polygon()
            );
        }

        let mut ob = world
            .add(Obj::Road(*r))
            .hitbox(road.get_thick_polygon());

        if focused_road == Some(*r) {
            let mut batch = GeomBatch::new();
            // Highlight the selected road
            batch.push(
                colors::HOVER.alpha(1.0),
                road.get_thick_polygon().to_outline(Distance::meters(3.0)),
            );

            // let convex = Polygon::convex_hull(hover_batch.clone().get_bounds());

            hover_batch.append(batch.clone());

            ob = ob.draw(batch);

        } else {
            ob = ob.drawn_in_master_batch();
        }

        ob.draw_hovered(hover_batch)
            .tooltip(Text::from(format!("{}", road_name(app, road))))
            .clickable()
            .build(ctx);
    }

    // TODO
    // Highlight the current prohibited destination roads
    // Highlight the potential prohibited destination roads

    world.initialize_hover(ctx);
    world
}

pub fn handle_world_outcome(
    ctx: &mut EventCtx,
    app: &mut App,
    outcome: WorldOutcome<Obj>,
    neighbourhood: &Neighbourhood,
) -> EditOutcome {
    // println!("TURN RESTRICTIONS: handle_world_outcome");

    match outcome {
        WorldOutcome::ClickedObject(Obj::Road(r)) => {
            // TODO - add logic based on which raod is clicked
            // Check if the ClickedObject is already highlighted
            // If so, then we should unhighlight it
            // If not and is one of the current prohibited destination roads,
            //      then we should remove that prohibited turn
            // If not and is one of the potential prohibited destination roads,
            //      then we should add that prohibited turn

            // let prev_selection = app.session.edit_mode

            let cursor_pt = ctx.canvas.get_cursor_in_map_space().unwrap();
            println!("click point {:?}", cursor_pt);

            if let EditMode::TurnRestrictions(ref prev_selection) = app.session.edit_mode {
                // let prev = prev_selection.unwrap();
                if prev_selection.is_some() {
                    let prev = prev_selection.as_ref().unwrap();
                    if r == prev.r {
                        println!("The same road has been clicked on twice {:?}", r);
                    } else {
                        println!("Two difference roads have been clicked on prev={:?}, new {:?}", prev.r, r);
                    }
                } else {
                    println!("No previous road selected. New selection {:?}", r);
                }
            }

            // app.per_map.map.get_i
            app.session.edit_mode = EditMode::TurnRestrictions(Some(FocusedTurns {
                r,
                i : app.per_map.map.get_r(r).dst_i
            }));
            println!("TURN RESTRICTIONS: handle_world_outcome - Clicked on Road {:?}", r);
            EditOutcome::UpdatePanelAndWorld
        }
        WorldOutcome::ClickedFreeSpace(_) => {
            app.session.edit_mode = EditMode::TurnRestrictions(None);
            println!("TURN RESTRICTIONS: handle_world_outcome - Clicked on FreeSpace");
            EditOutcome::UpdatePanelAndWorld
        }
        _ => EditOutcome::Nothing
    }
}

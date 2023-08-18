use std::collections::HashSet;

use geom::{Distance, Polygon, Pt2D};
use map_model::{RoadID, Map, EditRoad};
use osm2streets::RestrictionType;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, GeomBatch, Text, TextExt, Widget};
use widgetry::Color;

use super::{road_name, EditMode, EditOutcome, Obj};
use crate::logic::turn_restrictions::{destination_roads, restricted_destination_roads};
use crate::render::{colors, render_turn_restrictions};
use crate::{App, Neighbourhood};
use map_model::IntersectionID;

pub struct FocusedTurns {
    pub src_r: RoadID,
    pub i: IntersectionID,
    pub hull: Polygon,
    // pub permitted_t: Vec<Turn>,
    // pub prohibited_t: Vec<Turn>,
    pub permitted_t: HashSet<RoadID>,
    pub prohibited_t: HashSet<RoadID>,
}

impl FocusedTurns {
    pub fn new(r: RoadID, clicked_pt: Pt2D, map: &Map) -> Self {

        let dst_i = map.get_r(r).dst_i;
        let src_i = map.get_r(r).src_i;

        let dst_m = clicked_pt.fast_dist(map.get_i(dst_i).polygon.center());
        let src_m = clicked_pt.fast_dist(map.get_i(src_i).polygon.center());
        
        let i: IntersectionID;
        if dst_m > src_m {
            i = src_i;
        } else {
            i = dst_i;
        }

        let prohibited_t = restricted_destination_roads(map, r, Some(i));
        let permitted_t = destination_roads(map, r, Some(i));

        let mut ft = FocusedTurns {
            src_r: r,
            i,
            hull : Polygon::dummy(),
            permitted_t,
            prohibited_t,
        };

        ft.hull = hull_around_focused_turns(map, r,&ft.permitted_t, &ft.prohibited_t);
        ft
    }
}

fn hull_around_focused_turns(map: &Map, r: RoadID, permitted_t: &HashSet<RoadID>, prohibited_t: &HashSet<RoadID>) -> Polygon {

    let mut all_pt: Vec<Pt2D> = Vec::new();

    all_pt.extend(map.get_r(r).get_thick_polygon().get_outer_ring().clone().into_points());

    // Polygon::concave_hull(points, concavity)
    for t in permitted_t {
        all_pt.extend(map.get_r(*t).get_thick_polygon().get_outer_ring().clone().into_points());
    }

    for t in prohibited_t {
        all_pt.extend(map.get_r(*t).get_thick_polygon().get_outer_ring().clone().into_points());
    }

    // TODO the `200` value seems to work for some cases. But it is arbitary and there is no science
    // behind its the value. Need to work out what is an appropriate value _and why_.
    Polygon::concave_hull(all_pt, 200).unwrap_or(Polygon::dummy())
}


pub fn widget(ctx: &mut EventCtx, app: &App, focus: Option<&FocusedTurns>) -> Widget {
    match focus {
        Some(focus) => Widget::col(vec![format!(
            "Turn Restrictions from {}",
            app.per_map
                .map
                .get_r(focus.src_r)
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
    let focused_src_r = focus.as_ref().map(|f| f.src_r);

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

        let restricted_destinations = restricted_destination_roads(map, *r, None);

        // Account for one way streets when determining possible destinations
        // TODO This accounts for the oneway direction of the source street,
        // but not the oneway direction of the destination street
        let possible_destinations = destination_roads(map, road.id, None);

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
            let restricted_road = map.get_r(restricted_r);
            hover_batch.push(
                colors::TURN_PROHIBITED_DESTINATION,
                restricted_road.get_thick_polygon()
            );
        }

        let mut ob = world
            .add(Obj::Road(*r))
            .hitbox(road.get_thick_polygon());

        if focused_src_r == Some(*r) {
            let mut batch = GeomBatch::new();
            let focused_t = focus.as_ref().unwrap();

            // // Highlight the convex hull
            batch.push(
                Color::grey(0.4),
                focused_t.hull.clone(),
            );

            batch.push(
                Color::grey(0.2),
                focused_t.hull.to_outline(Distance::meters(3.0)),
            );

            // Highlight permitted destinations
            for pd in &focused_t.permitted_t {
                batch.push(
                    colors::TURN_PERMITTED_DESTINATION.alpha(1.0),
                    map.get_r(*pd).get_thick_polygon().to_outline(Distance::meters(3.0)),
                );
            }

            // Highlight prohibited destinations
            for pd in &focused_t.prohibited_t {
                batch.push(
                    colors::TURN_PROHIBITED_DESTINATION.alpha(1.0),
                    map.get_r(*pd).get_thick_polygon().to_outline(Distance::meters(3.0)),
                );
            }

            // Highlight the selected road
            batch.push(
                colors::HOVER.alpha(1.0),
                road.get_thick_polygon().to_outline(Distance::meters(3.0)),
            );

            // Highlight the selected intersection (the same color as the selected road)
            batch.push(
                colors::HOVER.alpha(1.0),
                map.get_i(focused_t.i).polygon.clone(),
            );
            batch.push(
                colors::HOVER.alpha(1.0),
                map.get_i(focused_t.i).polygon.to_outline(Distance::meters(3.0)),
            );

            

            hover_batch.append(batch.clone());

            ob = ob.draw(batch);

        } else {
            ob = ob.drawn_in_master_batch();
        }

        ob.draw_hovered(hover_batch)
            .tooltip(Text::from(format!("{} {}", road.id, road_name(app, road))))
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
    // println!("TURN RESTRICTIONS: handle_world_outcome");

    match outcome {
        WorldOutcome::ClickedObject(Obj::Road(r)) => {
            // TODO - add logic based on which road is clicked
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
                    if r == prev.src_r {
                        println!("The same road has been clicked on twice {:?}", r);
                    } else if prev.prohibited_t.contains(&r) || prev.permitted_t.contains(&r) {

                        // Copied from speed_limits.rs for reference
                        let mut edits = app.per_map.map.get_edits().clone();
                        // We are editing the previous road, not the most recently clicked road
                        let erc = app.per_map.map.edit_road_cmd(prev.src_r, |new| {
                            handle_edited_turn_restrictions(new, prev, r)
                        });
                        println!("erc={:?}", erc);
                        edits.commands.push(erc);
                        app.apply_edits(edits);
            
                        // Redraw the turn restriction symbols
                        // TODO find a better place for this. Forcing this here feels clunky. It seems like it would be
                        // cleaner to be part of the `Map` or `PerMap` object. There isn't a comparable layer (bus 
                        // routes etc), which are updated as a result of map edit.
                        app.per_map.draw_turn_restrictions = render_turn_restrictions(ctx, &app.per_map.map);
                        // Now clear the highlighted intersection/turns
                        app.session.edit_mode = EditMode::TurnRestrictions(None);
                        return EditOutcome::UpdateAll
                    } else {
                        println!("Two difference roads have been clicked on prev={:?}, new {:?}", prev.src_r, r);
                    }
                } else {
                    println!("No previous road selected. New selection {:?}", r);
                }
            }

            app.session.edit_mode = EditMode::TurnRestrictions(Some(FocusedTurns::new(r, cursor_pt, &app.per_map.map))); 
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

fn handle_edited_turn_restrictions(new: &mut EditRoad, ft: &FocusedTurns, dst_r: RoadID) {
    if ft.prohibited_t.contains(&dst_r) {
        println!("Remove existing banned turn from src={:?}, to dst {:?}", ft.src_r, dst_r);
        new.turn_restrictions.retain(|(_, r)| *r !=dst_r );
        new.complicated_turn_restrictions.retain(|(_, r)| *r !=dst_r );
    } else if ft.permitted_t.contains(&dst_r) {
        println!("Create new banned turn from src={:?}, to dst {:?}", ft.src_r, dst_r);
        new.turn_restrictions.push((RestrictionType::BanTurns, dst_r));
    } else {
        println!("Nothing to change src={:?}, to dst {:?}", ft.src_r, dst_r);
        return ()
    }
    ()
} 

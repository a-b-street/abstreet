use geom::Distance;
use map_model::{RoadID, EditRoad, Map};
use osm2streets::RestrictionType;
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{EventCtx, GeomBatch, Text, TextExt, Widget};
use widgetry::Color;

use super::{road_name, EditMode, EditOutcome, Obj};
use crate::logic::turn_restrictions::{FocusedTurns, possible_destination_roads, restricted_destination_roads};
use crate::render::{colors, render_turn_restrictions};
use crate::{App, Neighbourhood};
use map_model::IntersectionID;


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
        Some(focus) => {
            let road = app.per_map.map.get_r(focus.from_r);
            let restricted = focus.restricted_t.len();
            let permitted = focus.possible_t.len() - restricted;
            Widget::col(vec![
                format!("{} permitted and {} restricted",
                        permitted,
                        restricted,
                        ).text_widget(ctx),
                format!("turns from {} ", road.get_name(app.opts.language.as_ref())).text_widget(ctx),
                format!("at selected intersection").text_widget(ctx),
            ])
        },
        None => Widget::nothing(),
    }
}


pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
    focus: &Option<FocusedTurns>,
) -> World<Obj> {
    let mut world = World::new();

    if focus.is_none() {
        // Draw all roads as normal, with hoverover showing extant restrictions 
        let all_r_id = [
            &neighbourhood.perimeter_roads,
            &neighbourhood.interior_roads,
            &neighbourhood.connected_exterior_roads,
        ]
        .into_iter()
        .flatten();
    
        for r in all_r_id {
            build_turn_restriction_hoover_geom(*r, ctx, &mut world, app);
        }
    } else {
        let focused_t = focus.as_ref().unwrap();
        // Draw FocusTurns
        build_focused_turns_geom(focused_t, ctx, &mut world, app);
        // Create hoover geoms for each road connected to the FocusTurns
        build_turn_options_geom(focused_t, ctx, &mut world, app);
    }

    world.initialize_hover(ctx);
    world
}

/// Builds the hoover geom for showing turn restrictions when no FocusTurns are selected
fn build_turn_restriction_hoover_geom(r: RoadID, ctx: &mut EventCtx, world: &mut World<Obj>, app: &App) {
    let map = &app.per_map.map;
    let road = map.get_r(r);

    // Because we have "possible" destinations (rather than just "permitted") we must draw possible_destinations
    // first so that the distinct rendering of restricted_destinations shows on top of possible_destinations.
    let restricted_destinations = restricted_destination_roads(map, r, None);
    let possible_destinations = possible_destination_roads(map, road.id, None);

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
            colors::TURN_RESTRICTED_DESTINATION,
            restricted_road.get_thick_polygon()
        );
    }

    world
        .add(Obj::Road(r))
        .hitbox(road.get_thick_polygon())
        .drawn_in_master_batch()
        .draw_hovered(hover_batch)
        .tooltip(Text::from(format!("Click to edit turn restrictions from {}", road_name(app, road))))
        .clickable()
        .build(ctx);

}

/// Builds the geom representing the FocusTurns hull and intersection
fn build_focused_turns_geom(focused_t: &FocusedTurns, ctx: &mut EventCtx, world: &mut World<Obj>, app: &App) {
    let map = &app.per_map.map;
    let mut batch = GeomBatch::new();
    let from_road = map.get_r(focused_t.from_r);

    // Highlight the convex hull
    batch.push(
        Color::grey(0.4).alpha(0.8),
        focused_t.hull.clone(),
    );

    batch.push(
        Color::grey(0.2),
        focused_t.hull.to_outline(Distance::meters(2.0)),
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

    // add the convex hull using the IntersectionID
    world
        .add(Obj::Intersection(focused_t.i))
        .hitbox(focused_t.hull.clone())
        .draw(batch.clone())
        .draw_hovered(batch)
        .zorder(1)
        .tooltip(Text::from(format!("Edit restricted turn from {}", road_name(app, from_road))))
        .build(ctx);

}

/// Builds the geom representing each of the individual turn options (permitted and restricted)
/// within the FocusTurns.
fn build_turn_options_geom(focused_t: &FocusedTurns, ctx: &mut EventCtx, world: &mut World<Obj>, app: &App) {
    let map = &app.per_map.map;
    let from_road = map.get_r(focused_t.from_r);
    let from_road_name = road_name(app, from_road);

    let mut batch = GeomBatch::new();
    // Highlight the selected road
    batch.push(
        colors::HOVER.alpha(1.0),
        from_road.get_thick_polygon().to_outline(Distance::meters(3.0)),
    );

    world
        .add(Obj::Road(focused_t.from_r))
        .hitbox(from_road.get_thick_polygon())
        .draw(batch.clone())
        .draw_hovered(batch)
        .zorder(2)
        .tooltip(Text::from(format!("Edit restricted turn from {}", road_name(app, from_road))))
        .clickable()
        .build(ctx);

    // Highlight permitted destinations (Because we have "possible" but only what to show "permitted"
    // we need to draw these first, and then "restricted" on top - with a higher z-order.)
    for target_r in &focused_t.possible_t {
        if !&focused_t.restricted_t.contains(target_r) && target_r != &focused_t.from_r {
            build_individual_target_road_geom(
                *target_r,
                colors::TURN_PERMITTED_DESTINATION.alpha(1.0),
                colors::TURN_RESTRICTED_DESTINATION.alpha(1.0),
                3,
                format!("Add new turn restriction from '{}' to '{}'",
                                from_road_name, 
                                road_name(app, map.get_r(*target_r))),
                focused_t,
                ctx,
                world,
                map
            );
        }
    }

    // Highlight restricted destinations
    for target_r in &focused_t.restricted_t {
        build_individual_target_road_geom(
            *target_r,
            colors::TURN_RESTRICTED_DESTINATION.alpha(1.0),
            colors::TURN_PERMITTED_DESTINATION.alpha(1.0),
            4,
            format!("Remove turn restriction from '{}' to '{}'",
                            from_road_name, 
                            road_name(app, map.get_r(*target_r))),
            focused_t,
            ctx,
            world,
            map
        );
    }
}

fn build_individual_target_road_geom(
        target_r: RoadID,
        before_edit_color: Color,
        post_edit_color: Color,
        z_order: usize,
        tooltip: String,
        focused_t: &FocusedTurns,
        ctx: &mut EventCtx,
        world: &mut World<Obj>,
        map: &Map
    ) {
    // Highlight restricted destinations
    // Don't show U-Turns
    if target_r == focused_t.from_r {
        return;
    }

    let mut norm_batch = GeomBatch::new();
    let mut hover_batch = GeomBatch::new();
    let target_road = map.get_r(target_r);

    norm_batch.push(
        before_edit_color,
        target_road.get_thick_polygon().to_outline(Distance::meters(3.0)),
    );
    hover_batch.push(
        post_edit_color,
        target_road.get_thick_polygon(),
    );
    world
        .add(Obj::Road(target_r))
        .hitbox(target_road.get_thick_polygon())
        .draw(norm_batch)
        .draw_hovered(hover_batch)
        .zorder(z_order)
        .tooltip(Text::from(tooltip))
        .clickable()
        .build(ctx);
}


pub fn handle_world_outcome(
    ctx: &mut EventCtx,
    app: &mut App,
    outcome: WorldOutcome<Obj>,
) -> EditOutcome {
    match outcome {
        WorldOutcome::ClickedObject(Obj::Road(r)) => {
            // Check if the ClickedObject is already highlighted (ie there is a pre-existing FocusTurns)
            // If so, then we recreate the FocusTurns with the relevant clicked_point (as this
            //      is the easiest way to ensure the correct intersection is selected)
            // If not and is one of the current restricted destination roads,
            //      then we should remove that restricted turn
            // If not and is one of the permitted destination roads,
            //      then we should add that restricted turn
            let cursor_pt = ctx.canvas.get_cursor_in_map_space().unwrap();
            println!("click point {:?}", cursor_pt);

            if let EditMode::TurnRestrictions(ref prev_selection) = app.session.edit_mode {
                if prev_selection.is_some() {
                    let prev = prev_selection.as_ref().unwrap();
                    if r == prev.from_r {
                        debug!("The same road has been clicked on twice {:?}", r);
                    } else if prev.restricted_t.contains(&r) || prev.possible_t.contains(&r) {

                        let mut edits = app.per_map.map.get_edits().clone();
                        // We are editing the previous road, not the most recently clicked road
                        let erc = app.per_map.map.edit_road_cmd(prev.from_r, |new| {
                            handle_edited_turn_restrictions(new, prev, r)
                        });
                        debug!("erc={:?}", erc);
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
                        // Unreachable if the FocusTurns is is the only clickable objects in the world
                        debug!("Two difference roads have been clicked on prev={:?}, new {:?}", prev.from_r, r);
                    }
                } else {
                    debug!("No previous road selected. New selection {:?}", r);
                }
            }

            app.session.edit_mode = EditMode::TurnRestrictions(Some(FocusedTurns::new(r, cursor_pt, &app.per_map.map))); 
            debug!("TURN RESTRICTIONS: handle_world_outcome - Clicked on Road {:?}", r);
            EditOutcome::UpdatePanelAndWorld
        }
        WorldOutcome::ClickedFreeSpace(_) => {
            app.session.edit_mode = EditMode::TurnRestrictions(None);
            debug!("TURN RESTRICTIONS: handle_world_outcome - Clicked on FreeSpace");
            EditOutcome::UpdatePanelAndWorld
        }
        _ => EditOutcome::Nothing
    }
}

pub fn handle_edited_turn_restrictions(new: &mut EditRoad, ft: &FocusedTurns, target_r: RoadID) {
    if ft.restricted_t.contains(&target_r) {
        println!("Remove existing banned turn from src={:?}, to dst {:?}", ft.from_r, target_r);
        new.turn_restrictions.retain(|(_, r)| *r !=target_r );
        new.complicated_turn_restrictions.retain(|(_, r)| *r !=target_r );
    } else if ft.possible_t.contains(&target_r) {
        println!("Create new banned turn from src={:?}, to dst {:?}", ft.from_r, target_r);
        new.turn_restrictions.push((RestrictionType::BanTurns, target_r));
    } else {
        println!("Nothing to change src={:?}, to dst {:?}", ft.from_r, target_r);
        return ()
    }
    ()
} 

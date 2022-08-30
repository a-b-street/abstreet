use geom::Distance;
use map_model::{PathV2, RoadID};
use widgetry::mapspace::{World, WorldOutcome};
use widgetry::{Color, EventCtx, GeomBatch, Key, Line, Text, TextExt, Widget};

use super::{road_name, EditMode, EditOutcome, Obj};
use crate::{colors, App, Neighbourhood};

pub struct FocusedRoad {
    pub r: RoadID,
    pub paths: Vec<PathV2>,
    pub current_idx: usize,
}

pub fn widget(ctx: &mut EventCtx, app: &App, focus: Option<&FocusedRoad>) -> Widget {
    match focus {
        Some(focus) => Widget::col(vec![
            format!(
                "{} possible shortcuts cross {}",
                focus.paths.len(),
                app.map.get_r(focus.r).get_name(app.opts.language.as_ref()),
            )
            .text_widget(ctx),
            Widget::row(vec![
                ctx.style()
                    .btn_prev()
                    .disabled(focus.current_idx == 0)
                    .hotkey(Key::LeftArrow)
                    .build_widget(ctx, "previous shortcut"),
                Text::from(
                    Line(format!("{}/{}", focus.current_idx + 1, focus.paths.len())).secondary(),
                )
                .into_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_next()
                    .disabled(focus.current_idx == focus.paths.len() - 1)
                    .hotkey(Key::RightArrow)
                    .build_widget(ctx, "next shortcut"),
            ]),
        ]),
        None => Widget::col(vec![
            "Click a road to view shortcuts through it".text_widget(ctx)
        ]),
    }
}

pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
    focus: &Option<FocusedRoad>,
) -> World<Obj> {
    let map = &app.map;
    let mut world = World::bounded(map.get_bounds());
    let focused_road = focus.as_ref().map(|f| f.r);

    for r in &neighbourhood.orig_perimeter.interior {
        let road = map.get_r(*r);
        if focused_road == Some(*r) {
            let mut batch = GeomBatch::new();
            if let Ok(p) = road.get_thick_polygon().to_outline(Distance::meters(3.0)) {
                batch.push(Color::RED, p);
            }

            world
                .add(Obj::InteriorRoad(*r))
                .hitbox(road.get_thick_polygon())
                .draw(batch)
                .build(ctx);
        } else {
            world
                .add(Obj::InteriorRoad(*r))
                .hitbox(road.get_thick_polygon())
                .drawn_in_master_batch()
                .hover_color(colors::HOVER)
                .tooltip(Text::from(format!(
                    "{} possible shortcuts cross {}",
                    neighbourhood.shortcuts.count_per_road.get(*r),
                    road_name(app, road)
                )))
                .clickable()
                .build(ctx);
        }
    }

    if let Some(ref focus) = focus {
        let mut draw_path = GeomBatch::new();
        let path = &focus.paths[focus.current_idx];
        let color = app.cs.good_to_bad_red.0.last().unwrap().alpha(0.8);

        match path.trace_v2(&app.map) {
            Ok(poly) => {
                draw_path.push(color, poly);
            }
            Err(_) => {
                draw_path.extend(color, path.trace_all_polygons(&app.map));
            }
        }

        let first_pt = path.get_req().start.pt(&app.map);
        let last_pt = path.get_req().end.pt(&app.map);
        draw_path.append(map_gui::tools::start_marker(ctx, first_pt, 2.0));
        draw_path.append(map_gui::tools::goal_marker(ctx, last_pt, 2.0));

        world.draw_master_batch(ctx, draw_path);
    } else {
        world.draw_master_batch(ctx, neighbourhood.shortcuts.draw_heatmap(app));
    }

    world.initialize_hover(ctx);
    world
}

pub fn handle_world_outcome(
    app: &mut App,
    outcome: WorldOutcome<Obj>,
    neighbourhood: &Neighbourhood,
) -> EditOutcome {
    match outcome {
        WorldOutcome::ClickedObject(Obj::InteriorRoad(r)) => {
            let subset = neighbourhood.shortcuts.subset(neighbourhood, r);
            if subset.paths.is_empty() {
                EditOutcome::Nothing
            } else {
                app.session.edit_mode = EditMode::Shortcuts(Some(FocusedRoad {
                    r,
                    paths: subset.paths,
                    current_idx: 0,
                }));
                EditOutcome::UpdatePanelAndWorld
            }
        }
        WorldOutcome::ClickedFreeSpace(_) => {
            app.session.edit_mode = EditMode::Shortcuts(None);
            EditOutcome::UpdatePanelAndWorld
        }
        _ => EditOutcome::Nothing,
    }
}

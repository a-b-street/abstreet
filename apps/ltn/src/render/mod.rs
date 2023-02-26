mod cells;
pub mod colors;

use std::collections::HashSet;

use geom::Distance;
use map_model::osm::RoadRank;
use map_model::{AmenityType, Map, RoadID};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, RewriteColor};

use crate::App;

pub use cells::RenderCells;

pub fn draw_main_roads(ctx: &EventCtx, app: &App) -> Drawable {
    let mut roads = HashSet::new();
    for r in app.per_map.map.all_roads() {
        if r.get_rank() != RoadRank::Local {
            roads.insert(r.id);
        }
    }
    draw_roads(ctx, app, roads)
}

pub fn draw_boundary_roads(ctx: &EventCtx, app: &App) -> Drawable {
    let mut roads = HashSet::new();
    for info in app.partitioning().all_neighbourhoods().values() {
        for id in &info.block.perimeter.roads {
            roads.insert(id.road);
        }
    }
    draw_roads(ctx, app, roads)
}

fn draw_roads(ctx: &EventCtx, app: &App, roads: HashSet<RoadID>) -> Drawable {
    let mut batch = GeomBatch::new();
    let mut intersections = HashSet::new();
    for r in roads {
        let road = app.per_map.map.get_r(r);
        batch.push(colors::HIGHLIGHT_BOUNDARY, road.get_thick_polygon());
        intersections.insert(road.src_i);
        intersections.insert(road.dst_i);
    }
    for i in intersections {
        batch.push(
            colors::HIGHLIGHT_BOUNDARY,
            app.per_map.map.get_i(i).polygon.clone(),
        );
    }
    batch.build(ctx)
}

pub fn render_poi_icons(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();
    let school = GeomBatch::load_svg(ctx, "system/assets/map/school.svg")
        .scale(0.2)
        .color(RewriteColor::ChangeAll(Color::WHITE));

    for b in map.all_buildings() {
        if b.amenities.iter().any(|a| {
            let at = AmenityType::categorize(&a.amenity_type);
            at == Some(AmenityType::School) || at == Some(AmenityType::University)
        }) {
            batch.append(school.clone().centered_on(b.polygon.polylabel()));
        }
    }

    ctx.upload(batch)
}

pub fn render_bus_routes(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();
    for r in map.all_roads() {
        if map.get_bus_routes_on_road(r.id).is_empty() {
            continue;
        }
        // Draw dashed outlines surrounding the road
        let width = r.get_width();
        for pl in [
            r.center_pts.shift_left(width * 0.7),
            r.center_pts.shift_right(width * 0.7),
        ]
        .into_iter()
        .flatten()
        {
            batch.extend(
                *colors::BUS_ROUTE,
                pl.exact_dashed_polygons(
                    Distance::meters(2.0),
                    Distance::meters(5.0),
                    Distance::meters(2.0),
                ),
            );
        }
    }
    ctx.upload(batch)
}

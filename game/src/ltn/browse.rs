use std::collections::HashSet;

use anyhow::Result;

use abstutil::Timer;
use geom::{Distance, PolyLine, Pt2D};
use map_gui::tools::{CityPicker, DrawRoadLabels, Navigator, PopupMsg, URLManager};
use widgetry::mapspace::{ToggleZoomed, World, WorldOutcome};
use widgetry::{
    lctrl, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    Toggle, VerticalAlignment, Widget,
};

use super::{NeighborhoodID, Partitioning};
use crate::app::{App, Transition};
use crate::debug::DebugMode;

pub struct BrowseNeighborhoods {
    panel: Panel,
    world: World<NeighborhoodID>,
    labels: DrawRoadLabels,
    draw_all_filters: ToggleZoomed,
    draw_boundary_roads: ToggleZoomed,
}

impl BrowseNeighborhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        URLManager::update_url_map_name(app);

        let world = ctx.loading_screen("calculate neighborhoods", |ctx, timer| {
            detect_neighborhoods(ctx, app, timer)
        });
        let draw_all_filters = app.session.modal_filters.draw(ctx, &app.primary.map, None);

        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Click a neighborhood".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/search.svg")
                    .hotkey(Key::K)
                    .build_widget(ctx, "search")
                    .align_right(),
            ]),
            Toggle::checkbox(ctx, "highlight boundary roads", Key::H, true),
            ctx.style()
                .btn_outline
                .text("Export to GeoJSON")
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        Box::new(BrowseNeighborhoods {
            panel,
            world,
            labels: DrawRoadLabels::only_major_roads(),
            draw_all_filters,
            draw_boundary_roads: draw_boundary_roads(ctx, app),
        })
    }
}

impl State<App> for BrowseNeighborhoods {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Home" => {
                    return Transition::Pop;
                }
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            // TODO If we leave the LTN tool and change maps elsewhere, this won't
                            // work! Do we have per-map session state?
                            app.session.partitioning = Partitioning::empty();
                            Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
                        }),
                    ));
                }
                "search" => {
                    return Transition::Push(Navigator::new_state(ctx, app));
                }
                "Export to GeoJSON" => {
                    return Transition::Push(match export_geojson(app) {
                        Ok(path) => PopupMsg::new_state(
                            ctx,
                            "LTNs exported",
                            vec![format!("Data exported to {}", path)],
                        ),
                        Err(err) => {
                            PopupMsg::new_state(ctx, "Export failed", vec![err.to_string()])
                        }
                    });
                }
                _ => unreachable!(),
            }
        }

        if let WorldOutcome::ClickedObject(id) = self.world.event(ctx) {
            return Transition::Push(super::connectivity::Viewer::new_state(ctx, app, id));
        }

        if ctx.input.pressed(lctrl(Key::D)) {
            return Transition::Push(DebugMode::new_state(ctx, app));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        self.world.draw(g);
        self.draw_all_filters.draw(g);
        if self.panel.is_checked("highlight boundary roads") {
            self.draw_boundary_roads.draw(g);
        }
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }
}

fn detect_neighborhoods(
    ctx: &mut EventCtx,
    app: &mut App,
    timer: &mut Timer,
) -> World<NeighborhoodID> {
    // TODO Or if the map doesn't match? Do we take care of this in SessionState for anything?!
    if app.session.partitioning.neighborhoods.is_empty() {
        app.session.partitioning = Partitioning::seed_using_heuristics(app, timer);
    }

    let mut world = World::bounded(app.primary.map.get_bounds());
    for (id, (block, color)) in &app.session.partitioning.neighborhoods {
        world
            .add(*id)
            .hitbox(block.polygon.clone())
            .draw_color(color.alpha(0.5))
            .hover_outline(Color::BLACK, Distance::meters(5.0))
            .clickable()
            .build(ctx);
    }
    world
}

fn draw_boundary_roads(ctx: &EventCtx, app: &App) -> ToggleZoomed {
    let mut seen_roads = HashSet::new();
    let mut seen_borders = HashSet::new();
    let mut batch = ToggleZoomed::builder();
    for (block, _) in app.session.partitioning.neighborhoods.values() {
        for id in &block.perimeter.roads {
            let r = id.road;
            if seen_roads.contains(&r) {
                continue;
            }
            seen_roads.insert(r);
            let road = app.primary.map.get_r(r);
            batch
                .unzoomed
                .push(Color::RED.alpha(0.8), road.get_thick_polygon());
            batch
                .zoomed
                .push(Color::RED.alpha(0.5), road.get_thick_polygon());

            for i in [road.src_i, road.dst_i] {
                if seen_borders.contains(&i) {
                    continue;
                }
                seen_borders.insert(i);
                batch.unzoomed.push(
                    Color::RED.alpha(0.8),
                    app.primary.map.get_i(i).polygon.clone(),
                );
                batch.zoomed.push(
                    Color::RED.alpha(0.5),
                    app.primary.map.get_i(i).polygon.clone(),
                );
            }
        }
    }
    batch.build(ctx)
}

fn export_geojson(app: &App) -> Result<String> {
    if cfg!(target_arch = "wasm32") {
        bail!("Export only supported in the installed version");
    }

    use geo::algorithm::map_coords::MapCoordsInplace;
    use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
    use std::io::Write;

    let map = &app.primary.map;
    let mut features = Vec::new();

    // All neighborhood boundaries
    for (_, (block, color)) in &app.session.partitioning.neighborhoods {
        let mut feature = Feature {
            bbox: None,
            geometry: Some(block.polygon.to_geojson(None)),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("type", "neighborhood");
        feature.set_property("fill", color.as_hex());
        features.push(feature);
    }

    // TODO Cells per neighborhood -- contouring the gridded version is hard!

    // All modal filters
    for (r, dist) in &app.session.modal_filters.roads {
        let road = map.get_r(*r);
        if let Ok((pt, angle)) = road.center_pts.dist_along(*dist) {
            let road_width = road.get_width();
            let pl = PolyLine::must_new(vec![
                pt.project_away(0.8 * road_width, angle.rotate_degs(90.0)),
                pt.project_away(0.8 * road_width, angle.rotate_degs(-90.0)),
            ]);
            let mut feature = Feature {
                bbox: None,
                geometry: Some(pl.to_geojson(None)),
                id: None,
                properties: None,
                foreign_members: None,
            };
            feature.set_property("type", "road filter");
            feature.set_property("stroke", "red");
            features.push(feature);
        }
    }
    for (_, filter) in &app.session.modal_filters.intersections {
        let pl = filter.geometry(map).to_polyline();
        let mut feature = Feature {
            bbox: None,
            geometry: Some(pl.to_geojson(None)),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("type", "diagonal filter");
        feature.set_property("stroke", "red");
        features.push(feature);
    }

    // Transform to WGS84
    let gps_bounds = map.get_gps_bounds();
    for feature in &mut features {
        // geojson to geo
        // This could be a Polygon, MultiPolygon, LineString
        let mut geom: geo::Geometry<f64> = feature.geometry.take().unwrap().value.try_into()?;

        geom.map_coords_inplace(|c| {
            let gps = Pt2D::new(c.0, c.1).to_gps(gps_bounds);
            (gps.x(), gps.y())
        });

        // geo to geojson
        feature.geometry = Some(Geometry {
            bbox: None,
            value: Value::from(&geom),
            foreign_members: None,
        });
    }

    let gj = GeoJson::FeatureCollection(FeatureCollection {
        features,
        bbox: None,
        foreign_members: None,
    });

    // Don't use abstio::write_json; it writes to local storage in web, where we want to eventually
    // make the browser download something
    let path = format!("ltn_{}.geojson", map.get_name().map);
    let mut file = std::fs::File::create(&path)?;
    write!(file, "{}", serde_json::to_string_pretty(&gj)?)?;
    Ok(path)
}
